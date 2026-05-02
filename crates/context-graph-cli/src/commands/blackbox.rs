//! BlackBoxFix Phase 1 local ledger and CLI.
//!
//! Source of truth: SQLite table `ledger_records` in the user-provided
//! ledger database. Every mutating command writes only after redaction,
//! schema validation, and hash-chain verification have succeeded.

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use chrono::Utc;
use clap::{Args, Parser, Subcommand, ValueEnum};
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, RETRY_AFTER, USER_AGENT};
use reqwest::{Client, Url};
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::Serialize;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use tracing::info;
use uuid::Uuid;

const LEDGER_SCHEMA_VERSION: i64 = 2;
const REDACTION_POLICY_ID: &str = "redact_v1";
const SQLITE_CONNECTOR_ID: &str = "sqlite_local_v1";
const GIT_CONNECTOR_ID: &str = "git_local_v1";
const FS_CONNECTOR_ID: &str = "fs_local_v1";
const CMD_CONNECTOR_ID: &str = "cmd_local_v1";
const GITHUB_CONNECTOR_ID: &str = "github_v1";
const GHA_CONNECTOR_ID: &str = "gha_v1";
const GITHUB_API_BASE: &str = "https://api.github.com";
const GITHUB_API_VERSION: &str = "2022-11-28";
const GITHUB_USER_AGENT: &str = "contextgraph-blackbox/0.1";

#[derive(Parser, Debug)]
#[command(name = "blackbox")]
#[command(author = "Context Graph Team")]
#[command(version = "0.1.0")]
#[command(about = "BlackBoxFix local ledger recorder")]
pub struct BlackboxCli {
    /// Verbosity level (-v, -vv, -vvv)
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,

    #[command(subcommand)]
    pub command: BlackboxCommands,
}

#[derive(Subcommand, Debug)]
pub enum BlackboxCommands {
    /// Initialize a local append-only ledger database.
    Init(InitArgs),
    /// Test source-of-truth connectors.
    Connector {
        #[command(subcommand)]
        action: ConnectorCommands,
    },
    /// Record agent runs and tool calls.
    Record {
        #[command(subcommand)]
        action: RecordCommands,
    },
    /// Verify a tool call against local source-of-truth systems.
    Verify(VerifyArgs),
    /// Inspect physical ledger state.
    Inspect {
        #[command(subcommand)]
        action: InspectCommands,
    },
}

#[derive(Args, Debug)]
pub struct InitArgs {
    /// SQLite ledger database path.
    #[arg(long, env = "BLACKBOX_LEDGER")]
    pub ledger: PathBuf,
}

#[derive(Subcommand, Debug)]
pub enum ConnectorCommands {
    /// List available connectors.
    List,
    /// Test a connector against a physical local source.
    Test(ConnectorTestArgs),
}

#[derive(Args, Debug)]
pub struct ConnectorTestArgs {
    /// Connector id: git_local_v1, fs_local_v1, or cmd_local_v1.
    #[arg(long)]
    pub connector: String,

    /// Physical source path. For cmd_local_v1 this is the working directory.
    #[arg(long)]
    pub source: PathBuf,

    /// Command argv for cmd_local_v1. Pass each argument separately after --command.
    #[arg(long, num_args = 1.., allow_hyphen_values = true)]
    pub command: Vec<String>,
}

#[derive(Subcommand, Debug)]
pub enum RecordCommands {
    /// Record a new agent run.
    Run(RecordRunArgs),
    /// Record a tool call for an existing run.
    #[command(name = "tool-call")]
    ToolCall(RecordToolCallArgs),
}

#[derive(Args, Debug)]
pub struct RecordRunArgs {
    /// SQLite ledger database path.
    #[arg(long, env = "BLACKBOX_LEDGER")]
    pub ledger: PathBuf,

    /// Requested task/outcome for the run.
    #[arg(long)]
    pub task: String,

    /// Agent identifier.
    #[arg(long)]
    pub agent: String,

    /// Tenant identifier for local mode.
    #[arg(long, default_value = "tenant_local")]
    pub tenant: String,

    /// Optional model identifier.
    #[arg(long)]
    pub model: Option<String>,

    /// Task kind.
    #[arg(long, default_value = "local_agent_task")]
    pub task_kind: String,

    /// Risk tier.
    #[arg(long, default_value = "medium")]
    pub risk_tier: String,
}

#[derive(Args, Debug)]
pub struct RecordToolCallArgs {
    /// SQLite ledger database path.
    #[arg(long, env = "BLACKBOX_LEDGER")]
    pub ledger: PathBuf,

    /// Existing run id.
    #[arg(long)]
    pub run_id: String,

    /// Tool name.
    #[arg(long)]
    pub tool: String,

    /// JSON file containing the real tool input.
    #[arg(long)]
    pub input: PathBuf,

    /// Tenant identifier for local mode.
    #[arg(long, default_value = "tenant_local")]
    pub tenant: String,

    /// Agent identifier.
    #[arg(long, default_value = "agent_unknown")]
    pub agent: String,

    /// Optional model identifier.
    #[arg(long)]
    pub model: Option<String>,

    /// Capability identifier.
    #[arg(long, default_value = "local.tool")]
    pub capability: String,

    /// Execution status recorded for the call.
    #[arg(long, default_value = "ok", value_enum)]
    pub status: ToolExecutionStatus,

    /// Optional external request id from the called system.
    #[arg(long)]
    pub external_request_id: Option<String>,
}

#[derive(Args, Debug)]
pub struct VerifyArgs {
    /// SQLite ledger database path.
    #[arg(long, env = "BLACKBOX_LEDGER")]
    pub ledger: PathBuf,

    /// JSON verifier spec.
    #[arg(long)]
    pub input: PathBuf,

    /// Tenant identifier for local mode.
    #[arg(long, default_value = "tenant_local")]
    pub tenant: String,

    /// Verifier actor identifier.
    #[arg(long, default_value = "blackbox_verifier")]
    pub agent: String,

    /// Optional model identifier.
    #[arg(long)]
    pub model: Option<String>,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
pub enum ToolExecutionStatus {
    Ok,
    Failed,
    Blocked,
}

impl ToolExecutionStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Failed => "failed",
            Self::Blocked => "blocked",
        }
    }
}

#[derive(Subcommand, Debug)]
pub enum InspectCommands {
    /// Read ledger rows from SQLite and verify the hash chain.
    Ledger(InspectLedgerArgs),
}

#[derive(Args, Debug)]
pub struct InspectLedgerArgs {
    /// SQLite ledger database path.
    #[arg(long, env = "BLACKBOX_LEDGER")]
    pub ledger: PathBuf,

    /// Optional run id filter. Integrity is still verified across the full ledger.
    #[arg(long)]
    pub run_id: Option<String>,

    /// Maximum records to include in output after filtering.
    #[arg(long)]
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ErrorKind {
    InvalidInput,
    Io,
    Sqlite,
    Schema,
    LedgerMissing,
    Redaction,
    NotFound,
    TenantMismatch,
    ConnectorBlocked,
    VerifierFailed,
    HashChain,
}

#[derive(Debug)]
pub struct BlackboxError {
    kind: ErrorKind,
    error_code: &'static str,
    message: String,
    source_of_truth: Option<String>,
    field_path: Option<String>,
    remediation: String,
    connector: &'static str,
}

impl BlackboxError {
    fn new(
        kind: ErrorKind,
        error_code: &'static str,
        message: impl Into<String>,
        source_of_truth: Option<String>,
        field_path: Option<String>,
        remediation: impl Into<String>,
    ) -> Self {
        Self::new_with_connector(
            kind,
            error_code,
            message,
            source_of_truth,
            SQLITE_CONNECTOR_ID,
            field_path,
            remediation,
        )
    }

    fn new_with_connector(
        kind: ErrorKind,
        error_code: &'static str,
        message: impl Into<String>,
        source_of_truth: Option<String>,
        connector: &'static str,
        field_path: Option<String>,
        remediation: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            error_code,
            message: message.into(),
            source_of_truth,
            field_path,
            remediation: remediation.into(),
            connector,
        }
    }

    fn invalid(message: impl Into<String>, remediation: impl Into<String>) -> Self {
        Self::new(
            ErrorKind::InvalidInput,
            "BBX_INVALID_INPUT",
            message,
            None,
            None,
            remediation,
        )
    }

    fn invalid_field_for_ledger(
        path: &Path,
        field_path: impl Into<String>,
        message: impl Into<String>,
        remediation: impl Into<String>,
    ) -> Self {
        Self::new(
            ErrorKind::InvalidInput,
            "BBX_INVALID_INPUT",
            message,
            Some(source_of_truth(path)),
            Some(field_path.into()),
            remediation,
        )
    }

    fn io(path: &Path, action: &'static str, err: std::io::Error) -> Self {
        Self::new(
            ErrorKind::Io,
            "BBX_IO_ERROR",
            format!("{action}: {err}"),
            Some(source_of_truth(path)),
            None,
            "check the path, parent directory, and filesystem permissions",
        )
    }

    fn sqlite(path: &Path, action: &'static str, err: rusqlite::Error) -> Self {
        Self::new(
            ErrorKind::Sqlite,
            "BBX_SQLITE_ERROR",
            format!("{action}: {err}"),
            Some(source_of_truth(path)),
            None,
            "inspect the SQLite error, verify the ledger path, and rerun after fixing the database state",
        )
    }

    fn schema(path: &Path, message: impl Into<String>) -> Self {
        Self::schema_field(path, "schema", message)
    }

    fn schema_field(
        path: &Path,
        field_path: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self::new(
            ErrorKind::Schema,
            "BBX_SCHEMA_VALIDATION_FAILED",
            message,
            Some(source_of_truth(path)),
            Some(field_path.into()),
            "run `blackbox init --ledger PATH` against a new ledger or migrate the existing database explicitly",
        )
    }

    fn ledger_missing(path: &Path) -> Self {
        Self::new(
            ErrorKind::LedgerMissing,
            "BBX_LEDGER_NOT_INITIALIZED",
            format!("ledger is not initialized at {}", path.display()),
            Some(source_of_truth(path)),
            Some("--ledger".to_string()),
            "initialize the physical ledger first with `blackbox init --ledger PATH`",
        )
    }

    fn redaction(path: &Path, field_path: String, message: impl Into<String>) -> Self {
        Self::new(
            ErrorKind::Redaction,
            "BBX_REDACTION_VIOLATION",
            message,
            Some(source_of_truth(path)),
            Some(field_path),
            "move the sensitive value into an explicitly redacted field or remove it before recording",
        )
    }

    fn not_found(path: &Path, message: impl Into<String>) -> Self {
        Self::new(
            ErrorKind::NotFound,
            "BBX_RECORD_NOT_FOUND",
            message,
            Some(source_of_truth(path)),
            Some("--run-id".to_string()),
            "record the parent run first and retry with an existing run_id",
        )
    }

    fn tenant_mismatch(path: &Path, run_id: &str, expected: &str, actual: &str) -> Self {
        Self::new(
            ErrorKind::TenantMismatch,
            "BBX_TENANT_MISMATCH",
            format!(
                "run_id {run_id} belongs to tenant {expected}, but the tool-call was recorded for tenant {actual}"
            ),
            Some(source_of_truth(path)),
            Some("ledger_records.tenant_id".to_string()),
            "record the tool call with the parent run's tenant_id or start a new run for the requested tenant",
        )
    }

    fn hash_chain(path: &Path, field_path: String, message: impl Into<String>) -> Self {
        Self::new(
            ErrorKind::HashChain,
            "BBX_HASH_CHAIN_BROKEN",
            message,
            Some(source_of_truth(path)),
            Some(field_path),
            "treat the ledger as tampered or corrupt; restore from trusted backup or start a new ledger",
        )
    }

    fn verifier_spec(
        path: &Path,
        field_path: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self::new(
            ErrorKind::InvalidInput,
            "BBX_VERIFIER_SPEC_INVALID",
            message,
            Some(source_of_truth(path)),
            Some(field_path.into()),
            "fix the verifier spec JSON so required fields and types match the verifier contract",
        )
    }

    fn connector_blocked(
        error_code: &'static str,
        connector: &'static str,
        source_of_truth: String,
        field_path: impl Into<String>,
        message: impl Into<String>,
        remediation: impl Into<String>,
    ) -> Self {
        Self::new_with_connector(
            ErrorKind::ConnectorBlocked,
            error_code,
            message,
            Some(source_of_truth),
            connector,
            Some(field_path.into()),
            remediation,
        )
    }

    fn verifier_failed(
        error_code: &'static str,
        connector: &'static str,
        source_of_truth: String,
        field_path: impl Into<String>,
        message: impl Into<String>,
        remediation: impl Into<String>,
    ) -> Self {
        Self::new_with_connector(
            ErrorKind::VerifierFailed,
            error_code,
            message,
            Some(source_of_truth),
            connector,
            Some(field_path.into()),
            remediation,
        )
    }

    fn exit_code(&self) -> i32 {
        match self.kind {
            ErrorKind::HashChain => 2,
            _ => 1,
        }
    }

    fn to_json(&self) -> Value {
        json!({
            "status": "error",
            "error_code": self.error_code,
            "message": self.message,
            "source_of_truth": self.source_of_truth,
            "connector": self.connector,
            "field_path": self.field_path,
            "remediation": self.remediation,
        })
    }
}

impl std::fmt::Display for BlackboxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.error_code, self.message)
    }
}

impl std::error::Error for BlackboxError {}

type BbxResult<T> = Result<T, BlackboxError>;

pub async fn handle_blackbox_command(command: BlackboxCommands) -> i32 {
    match run_blackbox_command(command).await {
        Ok(output) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&output).expect("JSON output serializes")
            );
            0
        }
        Err(err) => {
            eprintln!(
                "{}",
                serde_json::to_string_pretty(&err.to_json()).expect("JSON error serializes")
            );
            err.exit_code()
        }
    }
}

async fn run_blackbox_command(command: BlackboxCommands) -> BbxResult<Value> {
    match command {
        BlackboxCommands::Init(args) => init_ledger(args),
        BlackboxCommands::Connector { action } => match action {
            ConnectorCommands::List => connector_list(),
            ConnectorCommands::Test(args) => connector_test(args).await,
        },
        BlackboxCommands::Record { action } => match action {
            RecordCommands::Run(args) => record_run(args),
            RecordCommands::ToolCall(args) => record_tool_call(args),
        },
        BlackboxCommands::Verify(args) => verify_local(args).await,
        BlackboxCommands::Inspect { action } => match action {
            InspectCommands::Ledger(args) => inspect_ledger(args),
        },
    }
}

fn connector_list() -> BbxResult<Value> {
    Ok(json!({
        "status": "ok",
        "connectors": [
            {
                "connector_id": GIT_CONNECTOR_ID,
                "source_kind": "git",
                "readback_specs": [
                    "git_branch_exists_v1",
                    "git_commit_exists_v1",
                    "git_file_diff_matches_patch_v1",
                    "git_working_tree_clean_v1"
                ],
                "fail_closed": true
            },
            {
                "connector_id": FS_CONNECTOR_ID,
                "source_kind": "filesystem",
                "readback_specs": ["fs_file_readback_v1"],
                "fail_closed": true
            },
            {
                "connector_id": CMD_CONNECTOR_ID,
                "source_kind": "command",
                "readback_specs": ["cmd_exit_zero_v1"],
                "fail_closed": true
            },
            {
                "connector_id": GITHUB_CONNECTOR_ID,
                "source_kind": "github",
                "readback_specs": [
                    "github_pr_state_v1",
                    "github_branch_head_matches_commit_v1",
                    "github_check_suite_passed_v1",
                    "github_issue_state_changed_v1"
                ],
                "fail_closed": true
            },
            {
                "connector_id": GHA_CONNECTOR_ID,
                "source_kind": "github_actions",
                "readback_specs": ["gha_workflow_run_completed_v1"],
                "fail_closed": true
            }
        ]
    }))
}

async fn connector_test(args: ConnectorTestArgs) -> BbxResult<Value> {
    require_non_empty_for_source("--connector", &args.connector)?;
    match args.connector.as_str() {
        GIT_CONNECTOR_ID => {
            if !args.command.is_empty() {
                return Err(BlackboxError::connector_blocked(
                    "CONNECTOR_TEST_SCHEMA_INVALID",
                    GIT_CONNECTOR_ID,
                    git_source_of_truth(&args.source, "connector_test"),
                    "--command",
                    "git_local_v1 connector test does not accept --command",
                    "remove --command and pass the repository path through --source",
                ));
            }
            let readback = git_repo_readback(&args.source)?;
            Ok(json!({
                "status": "ok",
                "connector": GIT_CONNECTOR_ID,
                "source_of_truth": git_source_of_truth(&args.source, "repository"),
                "readback": readback,
            }))
        }
        FS_CONNECTOR_ID => {
            if !args.command.is_empty() {
                return Err(BlackboxError::connector_blocked(
                    "CONNECTOR_TEST_SCHEMA_INVALID",
                    FS_CONNECTOR_ID,
                    file_source_of_truth(&args.source),
                    "--command",
                    "fs_local_v1 connector test does not accept --command",
                    "remove --command and pass the file path through --source",
                ));
            }
            let readback = fs_file_readback(&args.source)?;
            Ok(json!({
                "status": "ok",
                "connector": FS_CONNECTOR_ID,
                "source_of_truth": file_source_of_truth(&args.source),
                "readback": readback,
            }))
        }
        CMD_CONNECTOR_ID => {
            if args.command.is_empty() {
                return Err(BlackboxError::connector_blocked(
                    "CONNECTOR_TEST_SCHEMA_INVALID",
                    CMD_CONNECTOR_ID,
                    cmd_source_of_truth(&args.source, &args.command),
                    "--command",
                    "cmd_local_v1 connector test requires at least one --command value",
                    "pass the executable and each argument separately after --command",
                ));
            }
            let readback = cmd_readback(&args.source, &args.command, &[])?;
            Ok(json!({
                "status": "ok",
                "connector": CMD_CONNECTOR_ID,
                "source_of_truth": cmd_source_of_truth(&args.source, &args.command),
                "readback": readback,
            }))
        }
        GITHUB_CONNECTOR_ID => {
            if !args.command.is_empty() {
                return Err(BlackboxError::connector_blocked(
                    "CONNECTOR_TEST_SCHEMA_INVALID",
                    GITHUB_CONNECTOR_ID,
                    github_source_of_truth_from_locator(
                        &args.source.display().to_string(),
                        "repository",
                    ),
                    "--command",
                    "github_v1 connector test does not accept --command",
                    "remove --command and pass OWNER/REPO through --source",
                ));
            }
            let (owner, repo) = parse_github_repo_locator(&args.source.display().to_string())?;
            let readback = github_repo_readback(&owner, &repo).await?;
            Ok(json!({
                "status": "ok",
                "connector": GITHUB_CONNECTOR_ID,
                "source_of_truth": github_source_of_truth(&owner, &repo, "repository"),
                "readback": readback,
            }))
        }
        GHA_CONNECTOR_ID => {
            if !args.command.is_empty() {
                return Err(BlackboxError::connector_blocked(
                    "CONNECTOR_TEST_SCHEMA_INVALID",
                    GHA_CONNECTOR_ID,
                    github_source_of_truth_from_locator(
                        &args.source.display().to_string(),
                        "actions/runs",
                    ),
                    "--command",
                    "gha_v1 connector test does not accept --command",
                    "remove --command and pass OWNER/REPO through --source",
                ));
            }
            let (owner, repo) = parse_github_repo_locator(&args.source.display().to_string())?;
            let readback = gha_repo_readback(&owner, &repo).await?;
            Ok(json!({
                "status": "ok",
                "connector": GHA_CONNECTOR_ID,
                "source_of_truth": gha_source_of_truth(&owner, &repo, "actions/runs"),
                "readback": readback,
            }))
        }
        other => Err(BlackboxError::connector_blocked(
            "CONNECTOR_UNKNOWN",
            SQLITE_CONNECTOR_ID,
            "blackbox:connector_registry".to_string(),
            "--connector",
            format!("unknown connector_id {other}"),
            "run `blackbox connector list` and choose one of the advertised connector ids",
        )),
    }
}

fn init_ledger(args: InitArgs) -> BbxResult<Value> {
    validate_ledger_path(&args.ledger)?;
    if let Some(parent) = args.ledger.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .map_err(|err| BlackboxError::io(parent, "create ledger parent directory", err))?;
        }
    }

    let wiped_existing_ledger = wipe_ledger_on_schema_version_mismatch(&args.ledger)?;
    let mut conn = open_connection(&args.ledger)?;
    if table_exists(&conn, &args.ledger, "blackbox_metadata")?
        || table_exists(&conn, &args.ledger, "ledger_records")?
    {
        ensure_schema(&conn, &args.ledger)?;
    }
    initialize_schema(&mut conn, &args.ledger)?;
    let inspection = verify_ledger_integrity(&conn, &args.ledger)?;

    Ok(json!({
        "status": "ok",
        "command": "blackbox init",
        "ledger_path": args.ledger,
        "source_of_truth": source_of_truth(&args.ledger),
        "schema_version": LEDGER_SCHEMA_VERSION,
        "redaction_policy_id": REDACTION_POLICY_ID,
        "record_count": inspection.record_count,
        "ledger_tip_sha256": inspection.ledger_tip_sha256,
        "wiped_existing_ledger": wiped_existing_ledger,
    }))
}

fn record_run(args: RecordRunArgs) -> BbxResult<Value> {
    validate_ledger_path(&args.ledger)?;
    require_non_empty_for_ledger(&args.ledger, "--task", &args.task)?;
    require_non_empty_for_ledger(&args.ledger, "--agent", &args.agent)?;
    require_non_empty_for_ledger(&args.ledger, "--tenant", &args.tenant)?;
    require_non_empty_for_ledger(&args.ledger, "--task-kind", &args.task_kind)?;
    validate_risk_tier(&args.ledger, &args.risk_tier)?;

    let mut conn = open_initialized_ledger(&args.ledger)?;
    let run_id = prefixed_id("run");
    let record_id = prefixed_id("bbx_rec");
    let actor = actor_json(&args.agent, args.model.as_deref());
    let payload = json!({
        "record_kind": "agent_run",
        "run_id": run_id,
        "task": {
            "task_kind": args.task_kind,
            "requested_outcome": args.task,
            "risk_tier": args.risk_tier,
        },
        "runtime": {
            "framework": "cli",
            "adapter_version": env!("CARGO_PKG_VERSION"),
        },
        "status": "running",
        "summary": {
            "steps_total": 0,
            "tool_calls_total": 0,
            "verifier_passed": 0,
            "verifier_failed": 0,
            "verifier_blocked": 0,
        },
    });
    let redaction = RedactionReport::empty();

    let append = append_record(
        &mut conn,
        &args.ledger,
        NewRecord {
            record_id: record_id.clone(),
            record_kind: "agent_run".to_string(),
            tenant_id: args.tenant,
            run_id: Some(run_id.clone()),
            step_id: None,
            actor,
            payload,
            redaction,
        },
    )?;

    Ok(json!({
        "status": "ok",
        "record_id": record_id,
        "record_kind": "agent_run",
        "run_id": run_id,
        "ledger_sequence": append.sequence,
        "source_of_truth": source_of_truth(&args.ledger),
        "payload_sha256": append.payload_sha256,
        "previous_record_sha256": append.previous_record_sha256,
        "run_chain_sha256": append.run_chain_sha256,
        "record_sha256": append.record_sha256,
    }))
}

fn record_tool_call(args: RecordToolCallArgs) -> BbxResult<Value> {
    validate_ledger_path(&args.ledger)?;
    require_non_empty_for_ledger(&args.ledger, "--run-id", &args.run_id)?;
    require_non_empty_for_ledger(&args.ledger, "--tool", &args.tool)?;
    require_non_empty_for_ledger(&args.ledger, "--tenant", &args.tenant)?;
    require_non_empty_for_ledger(&args.ledger, "--agent", &args.agent)?;
    require_non_empty_for_ledger(&args.ledger, "--capability", &args.capability)?;

    let mut conn = open_initialized_ledger(&args.ledger)?;
    ensure_run_exists_for_tenant(&conn, &args.ledger, &args.run_id, &args.tenant)?;

    let input_text = fs::read_to_string(&args.input)
        .map_err(|err| BlackboxError::io(&args.input, "read tool input JSON", err))?;
    let input_value: Value = serde_json::from_str(&input_text).map_err(|err| {
        BlackboxError::new(
            ErrorKind::InvalidInput,
            "BBX_INPUT_JSON_INVALID",
            format!("tool input is not valid JSON: {err}"),
            Some(format!("file:{}", args.input.display())),
            Some("$".to_string()),
            "write valid JSON to the input file and retry",
        )
    })?;
    if !input_value.is_object() {
        return Err(BlackboxError::new(
            ErrorKind::InvalidInput,
            "BBX_INPUT_SCHEMA_INVALID",
            "tool input JSON must be an object",
            Some(format!("file:{}", args.input.display())),
            Some("$".to_string()),
            "wrap the tool input in a JSON object so field-level redaction can be verified",
        ));
    }

    let redacted = redact_tool_input(&args.ledger, &input_value)?;
    let canonical_redacted = canonical_json_bytes(&redacted.value)?;
    let input_digest = sha256_hex(&canonical_redacted);

    let tool_call_id = prefixed_id("tool");
    let record_id = prefixed_id("bbx_rec");
    let now = Utc::now().to_rfc3339();
    let actor = actor_json(&args.agent, args.model.as_deref());
    let payload = json!({
        "record_kind": "tool_call",
        "tool_call_id": tool_call_id,
        "run_id": args.run_id,
        "tool_name": args.tool,
        "capability_id": args.capability,
        "input_digest": input_digest,
        "input_redacted": redacted.value,
        "authorization": {
            "policy_decision_id": null,
            "mode": "local",
            "approved_by": null,
        },
        "execution": {
            "started_at": now,
            "finished_at": now,
            "status": args.status.as_str(),
            "external_request_id": args.external_request_id,
        },
    });

    let append = append_record(
        &mut conn,
        &args.ledger,
        NewRecord {
            record_id: record_id.clone(),
            record_kind: "tool_call".to_string(),
            tenant_id: args.tenant,
            run_id: Some(args.run_id.clone()),
            step_id: None,
            actor,
            payload,
            redaction: redacted.report,
        },
    )?;

    Ok(json!({
        "status": "ok",
        "record_id": record_id,
        "record_kind": "tool_call",
        "run_id": args.run_id,
        "tool_call_id": tool_call_id,
        "ledger_sequence": append.sequence,
        "source_of_truth": source_of_truth(&args.ledger),
        "payload_sha256": append.payload_sha256,
        "previous_record_sha256": append.previous_record_sha256,
        "run_chain_sha256": append.run_chain_sha256,
        "record_sha256": append.record_sha256,
        "redaction": append.redaction,
    }))
}

async fn verify_local(args: VerifyArgs) -> BbxResult<Value> {
    validate_ledger_path(&args.ledger)?;
    require_non_empty_for_ledger(&args.ledger, "--tenant", &args.tenant)?;
    require_non_empty_for_ledger(&args.ledger, "--agent", &args.agent)?;

    let input_text = fs::read_to_string(&args.input)
        .map_err(|err| BlackboxError::io(&args.input, "read verifier spec JSON", err))?;
    let spec_value: Value = serde_json::from_str(&input_text).map_err(|err| {
        BlackboxError::new(
            ErrorKind::InvalidInput,
            "BBX_VERIFIER_SPEC_INVALID",
            format!("verifier spec is not valid JSON: {err}"),
            Some(format!("file:{}", args.input.display())),
            Some("$".to_string()),
            "write valid verifier spec JSON and retry",
        )
    })?;
    if !spec_value.is_object() {
        return Err(BlackboxError::new(
            ErrorKind::InvalidInput,
            "BBX_VERIFIER_SPEC_INVALID",
            "verifier spec JSON must be an object",
            Some(format!("file:{}", args.input.display())),
            Some("$".to_string()),
            "wrap the verifier spec in a JSON object",
        ));
    }
    let redacted_spec = redact_tool_input(&args.ledger, &spec_value)?;
    let spec = VerifySpec::parse(&args.ledger, &spec_value)?;

    let mut conn = open_initialized_ledger(&args.ledger)?;
    ensure_tool_call_exists_for_run_tenant(
        &conn,
        &args.ledger,
        &spec.run_id,
        &spec.tool_call_id,
        &args.tenant,
    )?;

    let actor = actor_json(&args.agent, args.model.as_deref());
    let expected_delta_id = prefixed_id("expected");
    let expected_payload = json!({
        "record_kind": "expected_delta",
        "expected_delta_id": expected_delta_id,
        "tool_call_id": spec.tool_call_id,
        "verifier_id": spec.verifier_id,
        "source_of_truth": spec.source_of_truth_hint(),
        "expected_persisted_delta": spec.expected,
        "spec_redacted": redacted_spec.value,
        "recorded_at": Utc::now().to_rfc3339(),
    });
    let expected_append = append_record(
        &mut conn,
        &args.ledger,
        NewRecord {
            record_id: prefixed_id("bbx_rec"),
            record_kind: "expected_delta".to_string(),
            tenant_id: args.tenant.clone(),
            run_id: Some(spec.run_id.clone()),
            step_id: None,
            actor: actor.clone(),
            payload: expected_payload,
            redaction: redacted_spec.report,
        },
    )?;

    let verification = run_phase2_verifier(&spec).await;
    let readback_append = if let Some(readback_payload) = verification.readback_payload.clone() {
        Some(append_record(
            &mut conn,
            &args.ledger,
            NewRecord {
                record_id: prefixed_id("bbx_rec"),
                record_kind: "readback_snapshot".to_string(),
                tenant_id: args.tenant.clone(),
                run_id: Some(spec.run_id.clone()),
                step_id: None,
                actor: actor.clone(),
                payload: readback_payload,
                redaction: RedactionReport::empty(),
            },
        )?)
    } else {
        None
    };

    let verifier_result_id = prefixed_id("verify");
    let verifier_payload = json!({
        "record_kind": "verifier_result",
        "verifier_result_id": verifier_result_id,
        "tool_call_id": spec.tool_call_id,
        "verifier_id": spec.verifier_id,
        "status": verification.status,
        "expected_persisted_delta": spec.expected,
        "actual_readback": verification.actual_readback,
        "score": {
            "correctness": verification.correctness,
            "risk": verification.risk,
            "confidence": verification.confidence,
        },
        "error_code": verification.error_code,
        "error_message": verification.message,
        "field_path": verification.field_path,
        "remediation": verification.remediation,
        "source_of_truth": verification.source_of_truth,
        "connector": verification.connector,
        "recorded_at": Utc::now().to_rfc3339(),
    });
    let verifier_append = append_record(
        &mut conn,
        &args.ledger,
        NewRecord {
            record_id: prefixed_id("bbx_rec"),
            record_kind: "verifier_result".to_string(),
            tenant_id: args.tenant,
            run_id: Some(spec.run_id),
            step_id: None,
            actor,
            payload: verifier_payload,
            redaction: RedactionReport::empty(),
        },
    )?;

    let output = json!({
        "status": "ok",
        "command": "blackbox verify",
        "verifier_id": spec.verifier_id,
        "verifier_status": verification.status,
        "connector": verification.connector,
        "source_of_truth": verification.source_of_truth,
        "expected_delta_sequence": expected_append.sequence,
        "readback_snapshot_sequence": readback_append.as_ref().map(|append| append.sequence),
        "verifier_result_sequence": verifier_append.sequence,
        "expected_delta_record_sha256": expected_append.record_sha256,
        "readback_snapshot_record_sha256": readback_append.as_ref().map(|append| append.record_sha256.clone()),
        "verifier_result_record_sha256": verifier_append.record_sha256,
        "actual_readback": verification.actual_readback,
    });

    match verification.status {
        "passed" => Ok(output),
        "failed" => Err(BlackboxError::verifier_failed(
            verification.error_code.unwrap_or("VERIFIER_FAILED"),
            verification.connector,
            verification.source_of_truth,
            verification
                .field_path
                .as_deref()
                .unwrap_or("actual_readback")
                .to_string(),
            verification.message.as_deref().unwrap_or("verifier failed"),
            verification
                .remediation
                .as_deref()
                .unwrap_or("inspect the source of truth and rerun after correcting the state"),
        )),
        "blocked" => Err(BlackboxError::connector_blocked(
            verification.error_code.unwrap_or("VERIFIER_BLOCKED"),
            verification.connector,
            verification.source_of_truth,
            verification
                .field_path
                .as_deref()
                .unwrap_or("source")
                .to_string(),
            verification
                .message
                .as_deref()
                .unwrap_or("verifier was blocked"),
            verification
                .remediation
                .as_deref()
                .unwrap_or("restore source-of-truth access and rerun verification"),
        )),
        _ => Ok(output),
    }
}

fn inspect_ledger(args: InspectLedgerArgs) -> BbxResult<Value> {
    let conn = open_initialized_ledger(&args.ledger)?;
    let inspection = verify_ledger_integrity(&conn, &args.ledger)?;

    let mut records = inspection.records;
    if let Some(run_id) = &args.run_id {
        records.retain(|record| record.run_id.as_deref() == Some(run_id.as_str()));
    }
    if let Some(limit) = args.limit {
        records.truncate(limit);
    }

    Ok(json!({
        "status": "ok",
        "source_of_truth": source_of_truth(&args.ledger),
        "connector": SQLITE_CONNECTOR_ID,
        "integrity": {
            "status": "passed",
            "schema_version": LEDGER_SCHEMA_VERSION,
            "record_count": inspection.record_count,
            "ledger_tip_sha256": inspection.ledger_tip_sha256,
            "metadata_record_count": inspection.metadata_record_count,
            "metadata_tip_sha256": inspection.metadata_tip_sha256,
        },
        "records": records,
    }))
}

fn validate_ledger_path(path: &Path) -> BbxResult<()> {
    if path.as_os_str().is_empty() {
        return Err(BlackboxError::invalid(
            "--ledger must not be empty",
            "provide an explicit SQLite ledger path",
        ));
    }
    if path.exists() && path.is_dir() {
        return Err(BlackboxError::schema(
            path,
            "ledger path points to a directory, expected a SQLite database file",
        ));
    }
    Ok(())
}

fn require_non_empty_for_ledger(path: &Path, field: &'static str, value: &str) -> BbxResult<()> {
    if value.trim().is_empty() {
        return Err(BlackboxError::invalid_field_for_ledger(
            path,
            field,
            format!("{field} must not be empty"),
            "provide a non-empty value and retry",
        ));
    }
    Ok(())
}

fn validate_risk_tier(path: &Path, value: &str) -> BbxResult<()> {
    match value {
        "low" | "medium" | "high" => Ok(()),
        other => Err(BlackboxError::invalid_field_for_ledger(
            path,
            "--risk-tier",
            format!("--risk-tier must be one of low, medium, or high; found {other:?}"),
            "choose one of the supported risk tiers: low, medium, high",
        )),
    }
}

fn wipe_ledger_on_schema_version_mismatch(path: &Path) -> BbxResult<bool> {
    if !path.exists() {
        return Ok(false);
    }
    let conn = open_connection(path)?;
    if !table_exists(&conn, path, "blackbox_metadata")? {
        return Ok(false);
    }
    let version = get_metadata(&conn, path, "schema_version")?;
    drop(conn);
    match version.as_deref() {
        Some(version) if version == LEDGER_SCHEMA_VERSION.to_string() => Ok(false),
        Some(_) => {
            remove_ledger_file_set(path)?;
            Ok(true)
        }
        None => Err(BlackboxError::schema_field(
            path,
            "blackbox_metadata.schema_version",
            "blackbox_metadata exists but schema_version is missing",
        )),
    }
}

fn remove_ledger_file_set(path: &Path) -> BbxResult<()> {
    for candidate in ledger_file_set(path) {
        if candidate.exists() {
            fs::remove_file(&candidate)
                .map_err(|err| BlackboxError::io(&candidate, "wipe old schema ledger file", err))?;
        }
    }
    Ok(())
}

fn ledger_file_set(path: &Path) -> Vec<PathBuf> {
    vec![
        path.to_path_buf(),
        PathBuf::from(format!("{}-wal", path.display())),
        PathBuf::from(format!("{}-shm", path.display())),
    ]
}

fn open_connection(path: &Path) -> BbxResult<Connection> {
    let conn = Connection::open(path)
        .map_err(|err| BlackboxError::sqlite(path, "open SQLite ledger", err))?;
    conn.execute_batch(
        "PRAGMA foreign_keys = ON;
         PRAGMA journal_mode = WAL;
         PRAGMA synchronous = FULL;
         PRAGMA busy_timeout = 5000;",
    )
    .map_err(|err| BlackboxError::sqlite(path, "configure SQLite ledger pragmas", err))?;
    Ok(conn)
}

fn open_initialized_ledger(path: &Path) -> BbxResult<Connection> {
    validate_ledger_path(path)?;
    if !path.exists() {
        return Err(BlackboxError::ledger_missing(path));
    }
    let conn = open_connection(path)?;
    ensure_schema(&conn, path)?;
    verify_ledger_integrity(&conn, path)?;
    Ok(conn)
}

fn initialize_schema(conn: &mut Connection, path: &Path) -> BbxResult<()> {
    conn.execute_batch(
        "BEGIN IMMEDIATE;
         CREATE TABLE IF NOT EXISTS blackbox_metadata (
             key TEXT PRIMARY KEY,
             value TEXT NOT NULL
         );
         CREATE TABLE IF NOT EXISTS ledger_records (
             sequence INTEGER PRIMARY KEY AUTOINCREMENT,
             record_id TEXT NOT NULL UNIQUE,
             schema_version INTEGER NOT NULL,
             record_kind TEXT NOT NULL,
             tenant_id TEXT NOT NULL,
             run_id TEXT,
             step_id TEXT,
             created_at TEXT NOT NULL,
             actor_json TEXT NOT NULL,
             payload_json TEXT NOT NULL,
             redaction_json TEXT NOT NULL,
             payload_sha256 TEXT NOT NULL,
             previous_record_sha256 TEXT,
             run_previous_record_sha256 TEXT,
             run_chain_sha256 TEXT NOT NULL,
             record_sha256 TEXT NOT NULL UNIQUE
         );
         CREATE INDEX IF NOT EXISTS idx_ledger_records_run_id
             ON ledger_records(run_id, sequence);
         CREATE INDEX IF NOT EXISTS idx_ledger_records_kind
             ON ledger_records(record_kind, sequence);
         CREATE TRIGGER IF NOT EXISTS ledger_records_no_update
             BEFORE UPDATE ON ledger_records
             BEGIN
                 SELECT RAISE(ABORT, 'ledger_records is append-only; updates are forbidden');
             END;
         CREATE TRIGGER IF NOT EXISTS ledger_records_no_delete
             BEFORE DELETE ON ledger_records
             BEGIN
                 SELECT RAISE(ABORT, 'ledger_records is append-only; deletes are forbidden');
             END;
         INSERT OR IGNORE INTO blackbox_metadata(key, value)
             VALUES ('schema_version', '2');
         INSERT OR IGNORE INTO blackbox_metadata(key, value)
             VALUES ('ledger_id', lower(hex(randomblob(16))));
         INSERT OR IGNORE INTO blackbox_metadata(key, value)
             VALUES ('created_at', strftime('%Y-%m-%dT%H:%M:%fZ','now'));
         INSERT OR IGNORE INTO blackbox_metadata(key, value)
             VALUES ('redaction_policy_id', 'redact_v1');
         INSERT OR IGNORE INTO blackbox_metadata(key, value)
             VALUES ('ledger_record_count', '0');
         INSERT OR IGNORE INTO blackbox_metadata(key, value)
             VALUES ('ledger_tip_sha256', '');
         COMMIT;",
    )
    .map_err(|err| BlackboxError::sqlite(path, "initialize ledger schema", err))?;
    ensure_schema(conn, path)
}

fn ensure_schema(conn: &Connection, path: &Path) -> BbxResult<()> {
    validate_table_columns(conn, path, "blackbox_metadata", expected_metadata_columns())?;
    validate_table_columns(conn, path, "ledger_records", expected_ledger_columns())?;
    validate_index_columns(
        conn,
        path,
        "idx_ledger_records_run_id",
        &["run_id", "sequence"],
    )?;
    validate_index_columns(
        conn,
        path,
        "idx_ledger_records_kind",
        &["record_kind", "sequence"],
    )?;
    validate_unique_index_for_columns(conn, path, "ledger_records", &["record_id"])?;
    validate_unique_index_for_columns(conn, path, "ledger_records", &["record_sha256"])?;
    validate_trigger_sql(
        conn,
        path,
        "ledger_records_no_update",
        "ledger_records",
        EXPECTED_UPDATE_TRIGGER_SQL,
    )?;
    validate_trigger_sql(
        conn,
        path,
        "ledger_records_no_delete",
        "ledger_records",
        EXPECTED_DELETE_TRIGGER_SQL,
    )?;
    validate_metadata_value(
        conn,
        path,
        "schema_version",
        &LEDGER_SCHEMA_VERSION.to_string(),
    )?;
    validate_metadata_value(conn, path, "redaction_policy_id", REDACTION_POLICY_ID)?;
    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
struct ExpectedColumn {
    name: &'static str,
    data_type: &'static str,
    notnull: bool,
    pk: i64,
}

#[derive(Debug, PartialEq, Eq)]
struct ActualColumn {
    name: String,
    data_type: String,
    notnull: bool,
    pk: i64,
}

const EXPECTED_UPDATE_TRIGGER_SQL: &str = "CREATE TRIGGER ledger_records_no_update
BEFORE UPDATE ON ledger_records
BEGIN
SELECT RAISE(ABORT, 'ledger_records is append-only; updates are forbidden');
END";

const EXPECTED_DELETE_TRIGGER_SQL: &str = "CREATE TRIGGER ledger_records_no_delete
BEFORE DELETE ON ledger_records
BEGIN
SELECT RAISE(ABORT, 'ledger_records is append-only; deletes are forbidden');
END";

fn expected_metadata_columns() -> &'static [ExpectedColumn] {
    &[
        ExpectedColumn {
            name: "key",
            data_type: "TEXT",
            notnull: false,
            pk: 1,
        },
        ExpectedColumn {
            name: "value",
            data_type: "TEXT",
            notnull: true,
            pk: 0,
        },
    ]
}

fn expected_ledger_columns() -> &'static [ExpectedColumn] {
    &[
        ExpectedColumn {
            name: "sequence",
            data_type: "INTEGER",
            notnull: false,
            pk: 1,
        },
        ExpectedColumn {
            name: "record_id",
            data_type: "TEXT",
            notnull: true,
            pk: 0,
        },
        ExpectedColumn {
            name: "schema_version",
            data_type: "INTEGER",
            notnull: true,
            pk: 0,
        },
        ExpectedColumn {
            name: "record_kind",
            data_type: "TEXT",
            notnull: true,
            pk: 0,
        },
        ExpectedColumn {
            name: "tenant_id",
            data_type: "TEXT",
            notnull: true,
            pk: 0,
        },
        ExpectedColumn {
            name: "run_id",
            data_type: "TEXT",
            notnull: false,
            pk: 0,
        },
        ExpectedColumn {
            name: "step_id",
            data_type: "TEXT",
            notnull: false,
            pk: 0,
        },
        ExpectedColumn {
            name: "created_at",
            data_type: "TEXT",
            notnull: true,
            pk: 0,
        },
        ExpectedColumn {
            name: "actor_json",
            data_type: "TEXT",
            notnull: true,
            pk: 0,
        },
        ExpectedColumn {
            name: "payload_json",
            data_type: "TEXT",
            notnull: true,
            pk: 0,
        },
        ExpectedColumn {
            name: "redaction_json",
            data_type: "TEXT",
            notnull: true,
            pk: 0,
        },
        ExpectedColumn {
            name: "payload_sha256",
            data_type: "TEXT",
            notnull: true,
            pk: 0,
        },
        ExpectedColumn {
            name: "previous_record_sha256",
            data_type: "TEXT",
            notnull: false,
            pk: 0,
        },
        ExpectedColumn {
            name: "run_previous_record_sha256",
            data_type: "TEXT",
            notnull: false,
            pk: 0,
        },
        ExpectedColumn {
            name: "run_chain_sha256",
            data_type: "TEXT",
            notnull: true,
            pk: 0,
        },
        ExpectedColumn {
            name: "record_sha256",
            data_type: "TEXT",
            notnull: true,
            pk: 0,
        },
    ]
}

fn table_exists(conn: &Connection, path: &Path, table: &'static str) -> BbxResult<bool> {
    conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
        params![table],
        |row| row.get::<_, i64>(0),
    )
    .map(|count| count == 1)
    .map_err(|err| BlackboxError::sqlite(path, "inspect SQLite schema tables", err))
}

fn table_info_sql(table: &'static str) -> BbxResult<&'static str> {
    match table {
        "blackbox_metadata" => Ok("PRAGMA table_info(blackbox_metadata)"),
        "ledger_records" => Ok("PRAGMA table_info(ledger_records)"),
        _ => Err(BlackboxError::invalid(
            format!("unsupported schema table {table}"),
            "report this as a bug; schema validators must use known table names",
        )),
    }
}

fn validate_table_columns(
    conn: &Connection,
    path: &Path,
    table: &'static str,
    expected: &[ExpectedColumn],
) -> BbxResult<()> {
    let mut stmt = conn
        .prepare(table_info_sql(table)?)
        .map_err(|err| BlackboxError::sqlite(path, "prepare table schema inspection", err))?;
    let rows = stmt
        .query_map([], |row| {
            Ok(ActualColumn {
                name: row.get(1)?,
                data_type: row.get::<_, String>(2)?.to_ascii_uppercase(),
                notnull: row.get::<_, i64>(3)? != 0,
                pk: row.get(5)?,
            })
        })
        .map_err(|err| BlackboxError::sqlite(path, "inspect table schema columns", err))?;
    let actual = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| BlackboxError::sqlite(path, "decode table schema columns", err))?;
    let expected_actual = expected
        .iter()
        .map(|col| ActualColumn {
            name: col.name.to_string(),
            data_type: col.data_type.to_string(),
            notnull: col.notnull,
            pk: col.pk,
        })
        .collect::<Vec<_>>();
    if actual != expected_actual {
        return Err(BlackboxError::schema_field(
            path,
            format!("schema.tables.{table}.columns"),
            format!(
                "{table} columns do not match schema version {LEDGER_SCHEMA_VERSION}; expected {expected_actual:?}, found {actual:?}"
            ),
        ));
    }
    Ok(())
}

fn validate_index_columns(
    conn: &Connection,
    path: &Path,
    index: &'static str,
    expected_columns: &[&'static str],
) -> BbxResult<()> {
    let actual = index_columns(conn, path, index)?;
    let expected = expected_columns
        .iter()
        .map(|column| (*column).to_string())
        .collect::<Vec<_>>();
    if actual != expected {
        return Err(BlackboxError::schema_field(
            path,
            format!("schema.indexes.{index}.columns"),
            format!(
                "{index} columns do not match; expected {expected_columns:?}, found {actual:?}"
            ),
        ));
    }
    Ok(())
}

fn validate_unique_index_for_columns(
    conn: &Connection,
    path: &Path,
    table: &'static str,
    expected_columns: &[&'static str],
) -> BbxResult<()> {
    let pragma = match table {
        "ledger_records" => "PRAGMA index_list(ledger_records)",
        _ => {
            return Err(BlackboxError::invalid(
                format!("unsupported unique-index table {table}"),
                "report this as a bug; schema validators must use known table names",
            ));
        }
    };
    let mut stmt = conn
        .prepare(pragma)
        .map_err(|err| BlackboxError::sqlite(path, "prepare unique index inspection", err))?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)? != 0,
                row.get::<_, String>(3)?,
            ))
        })
        .map_err(|err| BlackboxError::sqlite(path, "inspect unique indexes", err))?;
    for row in rows {
        let (index_name, unique, origin) =
            row.map_err(|err| BlackboxError::sqlite(path, "decode unique index row", err))?;
        if !unique || origin != "u" {
            continue;
        }
        let columns = index_columns(conn, path, &index_name)?;
        let expected = expected_columns
            .iter()
            .map(|column| (*column).to_string())
            .collect::<Vec<_>>();
        if columns == expected {
            return Ok(());
        }
    }
    Err(BlackboxError::schema_field(
        path,
        format!("schema.tables.{table}.unique_indexes"),
        format!("missing UNIQUE constraint on {table} columns {expected_columns:?}"),
    ))
}

fn index_columns(conn: &Connection, path: &Path, index_name: &str) -> BbxResult<Vec<String>> {
    let mut stmt = conn
        .prepare("SELECT name FROM pragma_index_info(?1) ORDER BY seqno")
        .map_err(|err| BlackboxError::sqlite(path, "prepare index column inspection", err))?;
    let rows = stmt
        .query_map(params![index_name], |row| row.get::<_, String>(0))
        .map_err(|err| BlackboxError::sqlite(path, "inspect index columns", err))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|err| BlackboxError::sqlite(path, "decode index columns", err))
}

fn validate_trigger_sql(
    conn: &Connection,
    path: &Path,
    trigger: &'static str,
    table: &'static str,
    expected_sql: &str,
) -> BbxResult<()> {
    let found: Option<(String, String)> = conn
        .query_row(
            "SELECT tbl_name, sql FROM sqlite_master WHERE type = 'trigger' AND name = ?1",
            params![trigger],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|err| BlackboxError::sqlite(path, "inspect append-only trigger SQL", err))?;
    let Some((tbl_name, sql)) = found else {
        return Err(BlackboxError::schema_field(
            path,
            format!("schema.triggers.{trigger}"),
            format!("required append-only trigger {trigger} is missing"),
        ));
    };
    if tbl_name != table {
        return Err(BlackboxError::schema_field(
            path,
            format!("schema.triggers.{trigger}.tbl_name"),
            format!("trigger {trigger} is attached to {tbl_name}, expected {table}"),
        ));
    }
    if normalize_schema_sql(&sql) != normalize_schema_sql(expected_sql) {
        return Err(BlackboxError::schema_field(
            path,
            format!("schema.triggers.{trigger}.sql"),
            format!("trigger {trigger} SQL does not match the append-only guard for schema version {LEDGER_SCHEMA_VERSION}"),
        ));
    }
    Ok(())
}

fn validate_metadata_value(
    conn: &Connection,
    path: &Path,
    key: &'static str,
    expected: &str,
) -> BbxResult<()> {
    let actual = get_metadata(conn, path, key)?;
    if actual.as_deref() != Some(expected) {
        return Err(BlackboxError::schema_field(
            path,
            format!("blackbox_metadata.{key}"),
            format!("metadata {key} expected {expected:?}, found {actual:?}"),
        ));
    }
    Ok(())
}

fn normalize_schema_sql(sql: &str) -> String {
    sql.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim_end_matches(';')
        .to_ascii_lowercase()
}

#[derive(Debug)]
struct NewRecord {
    record_id: String,
    record_kind: String,
    tenant_id: String,
    run_id: Option<String>,
    step_id: Option<String>,
    actor: Value,
    payload: Value,
    redaction: RedactionReport,
}

#[derive(Debug)]
struct AppendResult {
    sequence: i64,
    payload_sha256: String,
    previous_record_sha256: Option<String>,
    run_chain_sha256: String,
    record_sha256: String,
    redaction: Value,
}

fn append_record(conn: &mut Connection, path: &Path, record: NewRecord) -> BbxResult<AppendResult> {
    require_non_empty_for_ledger(path, "record_kind", &record.record_kind)?;
    require_non_empty_for_ledger(path, "tenant_id", &record.tenant_id)?;

    let tx = conn
        .transaction()
        .map_err(|err| BlackboxError::sqlite(path, "begin ledger append transaction", err))?;
    tx.execute_batch("PRAGMA defer_foreign_keys = OFF;")
        .map_err(|err| BlackboxError::sqlite(path, "configure append transaction", err))?;
    verify_ledger_integrity_tx(&tx, path)?;

    let previous_record_sha256 = latest_record_hash(&tx, path)?;
    let run_previous_record_sha256 = match &record.run_id {
        Some(run_id) => latest_run_record_hash(&tx, path, run_id)?,
        None => None,
    };

    let created_at = Utc::now().to_rfc3339();
    let payload_json = canonical_json_string(&record.payload)?;
    let actor_json = canonical_json_string(&record.actor)?;
    let redaction_value = record.redaction.to_json();
    let redaction_json = canonical_json_string(&redaction_value)?;
    let payload_sha256 = sha256_hex(payload_json.as_bytes());

    let run_chain_sha256 = compute_run_chain_hash(
        &record,
        &created_at,
        &actor_json,
        &payload_sha256,
        &redaction_json,
        run_previous_record_sha256.as_deref(),
    )?;
    let record_sha256 = compute_record_hash(
        &record,
        &created_at,
        &actor_json,
        &payload_sha256,
        &redaction_json,
        previous_record_sha256.as_deref(),
        run_previous_record_sha256.as_deref(),
        &run_chain_sha256,
    )?;

    tx.execute(
        "INSERT INTO ledger_records (
             record_id, schema_version, record_kind, tenant_id, run_id, step_id,
             created_at, actor_json, payload_json, redaction_json, payload_sha256,
             previous_record_sha256, run_previous_record_sha256, run_chain_sha256,
             record_sha256
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
        params![
            record.record_id,
            LEDGER_SCHEMA_VERSION,
            record.record_kind,
            record.tenant_id,
            record.run_id,
            record.step_id,
            created_at,
            actor_json,
            payload_json,
            redaction_json,
            payload_sha256,
            previous_record_sha256,
            run_previous_record_sha256,
            run_chain_sha256,
            record_sha256,
        ],
    )
    .map_err(|err| BlackboxError::sqlite(path, "insert append-only ledger record", err))?;

    let sequence = tx.last_insert_rowid();
    let record_count: i64 = tx
        .query_row("SELECT COUNT(*) FROM ledger_records", [], |row| row.get(0))
        .map_err(|err| BlackboxError::sqlite(path, "count ledger records after append", err))?;
    set_metadata(&tx, path, "ledger_record_count", &record_count.to_string())?;
    set_metadata(&tx, path, "ledger_tip_sha256", &record_sha256)?;

    tx.commit()
        .map_err(|err| BlackboxError::sqlite(path, "commit ledger append transaction", err))?;

    info!(
        source_of_truth = %source_of_truth(path),
        sequence,
        record_sha256,
        "appended blackbox ledger record"
    );

    Ok(AppendResult {
        sequence,
        payload_sha256,
        previous_record_sha256,
        run_chain_sha256,
        record_sha256,
        redaction: redaction_value,
    })
}

fn set_metadata(
    tx: &Transaction<'_>,
    path: &Path,
    key: &'static str,
    value: &str,
) -> BbxResult<()> {
    tx.execute(
        "INSERT INTO blackbox_metadata(key, value)
         VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )
    .map_err(|err| BlackboxError::sqlite(path, "write ledger metadata", err))?;
    Ok(())
}

fn ensure_run_exists_for_tenant(
    conn: &Connection,
    path: &Path,
    run_id: &str,
    tenant_id: &str,
) -> BbxResult<()> {
    let parent_tenant: Option<String> = conn
        .query_row(
            "SELECT tenant_id FROM ledger_records
             WHERE record_kind = 'agent_run' AND run_id = ?1",
            params![run_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|err| BlackboxError::sqlite(path, "read run parent from ledger", err))?;
    let Some(parent_tenant) = parent_tenant else {
        return Err(BlackboxError::not_found(
            path,
            format!("run_id {run_id} does not exist in ledger_records"),
        ));
    };
    if parent_tenant != tenant_id {
        return Err(BlackboxError::tenant_mismatch(
            path,
            run_id,
            &parent_tenant,
            tenant_id,
        ));
    }
    Ok(())
}

fn ensure_tool_call_exists_for_run_tenant(
    conn: &Connection,
    path: &Path,
    run_id: &str,
    tool_call_id: &str,
    tenant_id: &str,
) -> BbxResult<()> {
    ensure_run_exists_for_tenant(conn, path, run_id, tenant_id)?;
    let found: Option<(String, String)> = conn
        .query_row(
            "SELECT tenant_id, run_id FROM ledger_records
             WHERE record_kind = 'tool_call'
               AND json_extract(payload_json, '$.tool_call_id') = ?1",
            params![tool_call_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|err| BlackboxError::sqlite(path, "read tool-call parent from ledger", err))?;
    let Some((tool_tenant, tool_run_id)) = found else {
        return Err(BlackboxError::new(
            ErrorKind::NotFound,
            "BBX_RECORD_NOT_FOUND",
            format!("tool_call_id {tool_call_id} does not exist in ledger_records"),
            Some(source_of_truth(path)),
            Some("$.tool_call_id".to_string()),
            "record the tool call first and retry with an existing tool_call_id",
        ));
    };
    if tool_run_id != run_id {
        return Err(BlackboxError::new(
            ErrorKind::NotFound,
            "BBX_RECORD_NOT_FOUND",
            format!(
                "tool_call_id {tool_call_id} belongs to run_id {tool_run_id}, not requested run_id {run_id}"
            ),
            Some(source_of_truth(path)),
            Some("$.run_id".to_string()),
            "verify the tool call under the run_id that owns it",
        ));
    }
    if tool_tenant != tenant_id {
        return Err(BlackboxError::tenant_mismatch(
            path,
            run_id,
            &tool_tenant,
            tenant_id,
        ));
    }
    Ok(())
}

#[derive(Debug)]
struct VerifySpec {
    verifier_id: String,
    run_id: String,
    tool_call_id: String,
    source: Value,
    expected: Value,
}

impl VerifySpec {
    fn parse(ledger_path: &Path, value: &Value) -> BbxResult<Self> {
        Ok(Self {
            verifier_id: required_string(ledger_path, value, "$.verifier_id")?,
            run_id: required_string(ledger_path, value, "$.run_id")?,
            tool_call_id: required_string(ledger_path, value, "$.tool_call_id")?,
            source: required_object_value(ledger_path, value, "$.source")?,
            expected: required_object_value(ledger_path, value, "$.expected")?,
        })
    }

    fn source_of_truth_hint(&self) -> String {
        match self.verifier_id.as_str() {
            "git_branch_exists_v1" => git_source_of_truth(
                Path::new(optional_str(&self.source, "repo").unwrap_or("")),
                &format!(
                    "refs/heads/{}",
                    optional_str(&self.expected, "branch").unwrap_or("")
                ),
            ),
            "git_commit_exists_v1" => git_source_of_truth(
                Path::new(optional_str(&self.source, "repo").unwrap_or("")),
                optional_str(&self.expected, "commit").unwrap_or("commit"),
            ),
            "git_file_diff_matches_patch_v1" => git_source_of_truth(
                Path::new(optional_str(&self.source, "repo").unwrap_or("")),
                optional_str(&self.expected, "path").unwrap_or("diff"),
            ),
            "git_working_tree_clean_v1" => git_source_of_truth(
                Path::new(optional_str(&self.source, "repo").unwrap_or("")),
                "working_tree",
            ),
            "cmd_exit_zero_v1" => {
                let command = optional_string_array(&self.expected, "command").unwrap_or_default();
                cmd_source_of_truth(
                    Path::new(optional_str(&self.source, "cwd").unwrap_or("")),
                    &command,
                )
            }
            "github_pr_state_v1" => github_source_of_truth_from_parts(
                optional_str(&self.source, "owner").unwrap_or(""),
                optional_str(&self.source, "repo").unwrap_or(""),
                &format!(
                    "pulls/{}",
                    self.expected
                        .get("pr_number")
                        .and_then(Value::as_u64)
                        .map(|number| number.to_string())
                        .unwrap_or_else(|| "unknown".to_string())
                ),
            ),
            "github_branch_head_matches_commit_v1" => github_source_of_truth_from_parts(
                optional_str(&self.source, "owner").unwrap_or(""),
                optional_str(&self.source, "repo").unwrap_or(""),
                &format!(
                    "heads/{}",
                    optional_str(&self.expected, "branch").unwrap_or("unknown")
                ),
            ),
            "github_issue_state_changed_v1" => github_source_of_truth_from_parts(
                optional_str(&self.source, "owner").unwrap_or(""),
                optional_str(&self.source, "repo").unwrap_or(""),
                &format!(
                    "issues/{}",
                    self.expected
                        .get("issue_number")
                        .and_then(Value::as_u64)
                        .map(|number| number.to_string())
                        .unwrap_or_else(|| "unknown".to_string())
                ),
            ),
            "github_check_suite_passed_v1" => github_source_of_truth_from_parts(
                optional_str(&self.source, "owner").unwrap_or(""),
                optional_str(&self.source, "repo").unwrap_or(""),
                &format!(
                    "commits/{}/check-suites",
                    optional_str(&self.expected, "ref").unwrap_or("unknown")
                ),
            ),
            "gha_workflow_run_completed_v1" => gha_source_of_truth_from_parts(
                optional_str(&self.source, "owner").unwrap_or(""),
                optional_str(&self.source, "repo").unwrap_or(""),
                &format!(
                    "actions/runs/{}",
                    self.expected
                        .get("run_id")
                        .and_then(Value::as_u64)
                        .map(|number| number.to_string())
                        .unwrap_or_else(|| "unknown".to_string())
                ),
            ),
            _ => "blackbox:unknown_verifier".to_string(),
        }
    }
}

#[derive(Debug)]
struct VerificationOutcome {
    status: &'static str,
    connector: &'static str,
    source_of_truth: String,
    readback_payload: Option<Value>,
    actual_readback: Value,
    correctness: Option<f64>,
    risk: f64,
    confidence: f64,
    error_code: Option<&'static str>,
    field_path: Option<String>,
    message: Option<String>,
    remediation: Option<String>,
}

async fn run_phase2_verifier(spec: &VerifySpec) -> VerificationOutcome {
    match spec.verifier_id.as_str() {
        "git_branch_exists_v1" => verify_git_branch_exists(spec),
        "git_commit_exists_v1" => verify_git_commit_exists(spec),
        "git_file_diff_matches_patch_v1" => verify_git_file_diff_matches_patch(spec),
        "git_working_tree_clean_v1" => verify_git_working_tree_clean(spec),
        "cmd_exit_zero_v1" => verify_cmd_exit_zero(spec),
        "github_pr_state_v1" => verify_github_pr_state(spec).await,
        "github_branch_head_matches_commit_v1" => {
            verify_github_branch_head_matches_commit(spec).await
        }
        "github_issue_state_changed_v1" => verify_github_issue_state_changed(spec).await,
        "github_check_suite_passed_v1" => verify_github_check_suite_passed(spec).await,
        "gha_workflow_run_completed_v1" => verify_gha_workflow_run_completed(spec).await,
        _ => VerificationOutcome::blocked(
            SQLITE_CONNECTOR_ID,
            "blackbox:verifier_registry".to_string(),
            "VERIFIER_SPEC_MISSING",
            "$.verifier_id",
            "verifier_id is not registered",
            "choose a registered Phase 2 or Phase 3 verifier id",
            None,
        ),
    }
}

impl VerificationOutcome {
    fn passed(
        connector: &'static str,
        source_of_truth: String,
        readback_payload: Value,
        actual_readback: Value,
    ) -> Self {
        Self {
            status: "passed",
            connector,
            source_of_truth,
            readback_payload: Some(readback_payload),
            actual_readback,
            correctness: Some(1.0),
            risk: 0.2,
            confidence: 0.99,
            error_code: None,
            field_path: None,
            message: None,
            remediation: None,
        }
    }

    fn failed(
        connector: &'static str,
        source_of_truth: String,
        error_code: &'static str,
        field_path: impl Into<String>,
        message: impl Into<String>,
        remediation: impl Into<String>,
        readback_payload: Value,
        actual_readback: Value,
    ) -> Self {
        Self {
            status: "failed",
            connector,
            source_of_truth,
            readback_payload: Some(readback_payload),
            actual_readback,
            correctness: Some(0.0),
            risk: 0.4,
            confidence: 0.99,
            error_code: Some(error_code),
            field_path: Some(field_path.into()),
            message: Some(message.into()),
            remediation: Some(remediation.into()),
        }
    }

    fn blocked(
        connector: &'static str,
        source_of_truth: String,
        error_code: &'static str,
        field_path: impl Into<String>,
        message: impl Into<String>,
        remediation: impl Into<String>,
        readback_payload: Option<Value>,
    ) -> Self {
        let actual_readback = readback_payload
            .as_ref()
            .and_then(|payload| payload.get("observed_state").cloned())
            .unwrap_or_else(|| json!({ "blocked": true }));
        Self {
            status: "blocked",
            connector,
            source_of_truth,
            readback_payload,
            actual_readback,
            correctness: None,
            risk: 0.5,
            confidence: 0.0,
            error_code: Some(error_code),
            field_path: Some(field_path.into()),
            message: Some(message.into()),
            remediation: Some(remediation.into()),
        }
    }
}

#[derive(Debug)]
struct ProcessReadback {
    exit_code: i32,
    stdout: String,
    stderr: String,
}

#[derive(Debug)]
struct GithubApiReadback {
    status: u16,
    url: String,
    body: Value,
    body_sha256: String,
    rate_limit: Value,
    request_id: Option<String>,
    etag: Option<String>,
}

fn required_string(
    ledger_path: &Path,
    value: &Value,
    field_path: &'static str,
) -> BbxResult<String> {
    value
        .pointer(json_pointer_from_field_path(field_path).as_str())
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .filter(|text| !text.trim().is_empty())
        .ok_or_else(|| {
            BlackboxError::verifier_spec(
                ledger_path,
                field_path,
                format!("{field_path} is required and must be a non-empty string"),
            )
        })
}

fn required_object_value(
    ledger_path: &Path,
    value: &Value,
    field_path: &'static str,
) -> BbxResult<Value> {
    let item = value
        .pointer(json_pointer_from_field_path(field_path).as_str())
        .ok_or_else(|| {
            BlackboxError::verifier_spec(
                ledger_path,
                field_path,
                format!("{field_path} is required"),
            )
        })?;
    if !item.is_object() {
        return Err(BlackboxError::verifier_spec(
            ledger_path,
            field_path,
            format!("{field_path} must be a JSON object"),
        ));
    }
    Ok(item.clone())
}

fn json_pointer_from_field_path(field_path: &str) -> String {
    if field_path == "$" {
        return String::new();
    }
    let trimmed = field_path.trim_start_matches("$.");
    format!("/{}", trimmed.replace('.', "/"))
}

fn optional_str<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

fn optional_string_array(value: &Value, key: &str) -> Option<Vec<String>> {
    value.get(key).and_then(Value::as_array).map(|items| {
        items
            .iter()
            .filter_map(Value::as_str)
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>()
    })
}

fn required_expected_string(
    spec: &VerifySpec,
    key: &'static str,
    field_path: &'static str,
    connector: &'static str,
    source_of_truth: &str,
    allow_empty: bool,
) -> Result<String, VerificationOutcome> {
    match spec.expected.get(key) {
        Some(Value::String(value)) if allow_empty || !value.trim().is_empty() => Ok(value.clone()),
        Some(Value::String(_)) | None => Err(VerificationOutcome::blocked(
            connector,
            source_of_truth.to_string(),
            "VERIFIER_SPEC_MISSING_FIELD",
            field_path,
            format!(
                "{} requires {field_path} as {} JSON string",
                spec.verifier_id,
                if allow_empty { "a" } else { "a non-empty" }
            ),
            format!("set {field_path} to the expected string value"),
            None,
        )),
        Some(_) => Err(VerificationOutcome::blocked(
            connector,
            source_of_truth.to_string(),
            "VERIFIER_SPEC_FIELD_TYPE",
            field_path,
            format!(
                "{} requires {field_path} to be a JSON string",
                spec.verifier_id
            ),
            format!("replace {field_path} with a JSON string"),
            None,
        )),
    }
}

fn required_expected_bool(
    spec: &VerifySpec,
    key: &'static str,
    field_path: &'static str,
    connector: &'static str,
    source_of_truth: &str,
) -> Result<bool, VerificationOutcome> {
    match spec.expected.get(key) {
        Some(Value::Bool(value)) => Ok(*value),
        None => Err(VerificationOutcome::blocked(
            connector,
            source_of_truth.to_string(),
            "VERIFIER_SPEC_MISSING_FIELD",
            field_path,
            format!(
                "{} requires {field_path} as a JSON boolean",
                spec.verifier_id
            ),
            format!("set {field_path} to true or false"),
            None,
        )),
        Some(_) => Err(VerificationOutcome::blocked(
            connector,
            source_of_truth.to_string(),
            "VERIFIER_SPEC_FIELD_TYPE",
            field_path,
            format!(
                "{} requires {field_path} to be a JSON boolean",
                spec.verifier_id
            ),
            format!("replace {field_path} with true or false"),
            None,
        )),
    }
}

fn required_source_string(
    spec: &VerifySpec,
    key: &'static str,
    field_path: &'static str,
    connector: &'static str,
    source_of_truth: &str,
) -> Result<String, VerificationOutcome> {
    match spec.source.get(key) {
        Some(Value::String(value)) if !value.trim().is_empty() => Ok(value.clone()),
        Some(Value::String(_)) | None => Err(VerificationOutcome::blocked(
            connector,
            source_of_truth.to_string(),
            "VERIFIER_SPEC_MISSING_FIELD",
            field_path,
            format!(
                "{} requires {field_path} as a non-empty JSON string",
                spec.verifier_id
            ),
            format!("set {field_path} to the source value"),
            None,
        )),
        Some(_) => Err(VerificationOutcome::blocked(
            connector,
            source_of_truth.to_string(),
            "VERIFIER_SPEC_FIELD_TYPE",
            field_path,
            format!(
                "{} requires {field_path} to be a JSON string",
                spec.verifier_id
            ),
            format!("replace {field_path} with a JSON string"),
            None,
        )),
    }
}

fn required_expected_u64(
    spec: &VerifySpec,
    key: &'static str,
    field_path: &'static str,
    connector: &'static str,
    source_of_truth: &str,
) -> Result<u64, VerificationOutcome> {
    match spec.expected.get(key) {
        Some(Value::Number(value)) => value.as_u64().ok_or_else(|| {
            VerificationOutcome::blocked(
                connector,
                source_of_truth.to_string(),
                "VERIFIER_SPEC_FIELD_TYPE",
                field_path,
                format!(
                    "{} requires {field_path} to be a non-negative integer",
                    spec.verifier_id
                ),
                format!("replace {field_path} with a non-negative integer"),
                None,
            )
        }),
        None => Err(VerificationOutcome::blocked(
            connector,
            source_of_truth.to_string(),
            "VERIFIER_SPEC_MISSING_FIELD",
            field_path,
            format!(
                "{} requires {field_path} as a non-negative integer",
                spec.verifier_id
            ),
            format!("set {field_path} to the expected integer value"),
            None,
        )),
        Some(_) => Err(VerificationOutcome::blocked(
            connector,
            source_of_truth.to_string(),
            "VERIFIER_SPEC_FIELD_TYPE",
            field_path,
            format!(
                "{} requires {field_path} to be a non-negative integer",
                spec.verifier_id
            ),
            format!("replace {field_path} with a non-negative integer"),
            None,
        )),
    }
}

fn expected_string_array(
    spec: &VerifySpec,
    key: &'static str,
    field_path: &'static str,
    connector: &'static str,
    source_of_truth: &str,
    required_non_empty: bool,
) -> Result<Vec<String>, VerificationOutcome> {
    let Some(value) = spec.expected.get(key) else {
        if required_non_empty {
            return Err(VerificationOutcome::blocked(
                connector,
                source_of_truth.to_string(),
                "VERIFIER_SPEC_MISSING_FIELD",
                field_path,
                format!(
                    "{} requires {field_path} as a JSON string array",
                    spec.verifier_id
                ),
                format!("set {field_path} to an array of JSON strings"),
                None,
            ));
        }
        return Ok(Vec::new());
    };
    let Some(items) = value.as_array() else {
        return Err(VerificationOutcome::blocked(
            connector,
            source_of_truth.to_string(),
            "VERIFIER_SPEC_FIELD_TYPE",
            field_path,
            format!(
                "{} requires {field_path} to be a JSON array",
                spec.verifier_id
            ),
            format!("replace {field_path} with an array of JSON strings"),
            None,
        ));
    };
    if required_non_empty && items.is_empty() {
        return Err(VerificationOutcome::blocked(
            connector,
            source_of_truth.to_string(),
            "VERIFIER_SPEC_MISSING_FIELD",
            field_path,
            format!(
                "{} requires {field_path} to contain at least one string",
                spec.verifier_id
            ),
            format!(
                "add the executable and each argument as separate JSON strings in {field_path}"
            ),
            None,
        ));
    }
    let mut parsed = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        let Some(text) = item.as_str() else {
            return Err(VerificationOutcome::blocked(
                connector,
                source_of_truth.to_string(),
                "VERIFIER_SPEC_FIELD_TYPE",
                format!("{field_path}[{index}]"),
                format!(
                    "{} requires every {field_path} item to be a JSON string",
                    spec.verifier_id
                ),
                format!("replace {field_path}[{index}] with a JSON string"),
                None,
            ));
        };
        if required_non_empty && index == 0 && text.trim().is_empty() {
            return Err(VerificationOutcome::blocked(
                connector,
                source_of_truth.to_string(),
                "VERIFIER_SPEC_MISSING_FIELD",
                format!("{field_path}[{index}]"),
                format!(
                    "{} requires the command executable to be non-empty",
                    spec.verifier_id
                ),
                format!("set {field_path}[0] to an executable name or path"),
                None,
            ));
        }
        parsed.push(text.to_string());
    }
    Ok(parsed)
}

fn require_non_empty_for_source(field: &'static str, value: &str) -> BbxResult<()> {
    if value.trim().is_empty() {
        return Err(BlackboxError::new(
            ErrorKind::InvalidInput,
            "BBX_INVALID_INPUT",
            format!("{field} must not be empty"),
            None,
            Some(field.to_string()),
            "provide a non-empty value and retry",
        ));
    }
    Ok(())
}

fn git_repo_readback(repo: &Path) -> BbxResult<Value> {
    if repo.as_os_str().is_empty() {
        return Err(BlackboxError::connector_blocked(
            "GIT_REPO_PATH_MISSING",
            GIT_CONNECTOR_ID,
            git_source_of_truth(repo, "repository"),
            "source.repo",
            "Git repository path is empty",
            "provide a non-empty source.repo path",
        ));
    }
    if !repo.exists() || !repo.is_dir() {
        return Err(BlackboxError::connector_blocked(
            "GIT_REPO_NOT_FOUND",
            GIT_CONNECTOR_ID,
            git_source_of_truth(repo, "repository"),
            "source.repo",
            "Git repository directory does not exist",
            "create the repository directory or pass the correct source.repo path",
        ));
    }
    let git_dir = run_git(repo, &["rev-parse", "--git-dir"])?;
    if git_dir.exit_code != 0 {
        return Err(BlackboxError::connector_blocked(
            "GIT_REPO_NOT_FOUND",
            GIT_CONNECTOR_ID,
            git_source_of_truth(repo, "repository"),
            "source.repo",
            "source.repo is not a readable Git repository",
            "initialize the directory with git init or pass a valid repository path",
        ));
    }
    let top = run_git(repo, &["rev-parse", "--show-toplevel"])?;
    let head = run_git(repo, &["rev-parse", "--verify", "--quiet", "HEAD"])?;
    let status = run_git(repo, &["status", "--porcelain=v1", "--untracked-files=all"])?;
    let git_version = run_process(
        Path::new("."),
        &["git".to_string(), "--version".to_string()],
    )
    .map_err(|err| {
        BlackboxError::connector_blocked(
            "GIT_EXEC_FAILED",
            GIT_CONNECTOR_ID,
            git_source_of_truth(repo, "repository"),
            "git.version",
            format!("failed to execute git --version: {err}"),
            "install Git and ensure the git executable is available on PATH",
        )
    })?;
    if git_version.exit_code != 0 {
        return Err(BlackboxError::connector_blocked(
            "GIT_EXEC_FAILED",
            GIT_CONNECTOR_ID,
            git_source_of_truth(repo, "repository"),
            "git.version",
            format!("git --version exited non-zero: {}", git_version.stderr),
            "install a working Git executable and retry",
        ));
    }
    Ok(json!({
        "repo": repo.display().to_string(),
        "git_dir": git_dir.stdout.trim(),
        "worktree_root": top.stdout.trim(),
        "head": if head.exit_code == 0 { Value::String(head.stdout.trim().to_string()) } else { Value::Null },
        "status_porcelain_v1": status.stdout,
        "git_version": git_version.stdout.trim().to_string(),
    }))
}

fn fs_file_readback(path: &Path) -> BbxResult<Value> {
    if path.as_os_str().is_empty() {
        return Err(BlackboxError::connector_blocked(
            "FS_FILE_PATH_MISSING",
            FS_CONNECTOR_ID,
            file_source_of_truth(path),
            "source",
            "filesystem source path is empty",
            "provide a non-empty file path",
        ));
    }
    if !path.exists() || !path.is_file() {
        return Err(BlackboxError::connector_blocked(
            "FS_FILE_NOT_FOUND",
            FS_CONNECTOR_ID,
            file_source_of_truth(path),
            "source",
            "filesystem source file does not exist",
            "create the file or pass the correct source path",
        ));
    }
    let bytes = fs::read(path).map_err(|err| {
        BlackboxError::connector_blocked(
            "FS_FILE_READ_FAILED",
            FS_CONNECTOR_ID,
            file_source_of_truth(path),
            "source",
            format!("failed to read filesystem source file: {err}"),
            "fix file permissions and retry",
        )
    })?;
    Ok(json!({
        "path": path.display().to_string(),
        "size_bytes": bytes.len(),
        "sha256": sha256_hex(&bytes),
        "utf8": String::from_utf8(bytes).ok(),
        "read_at": Utc::now().to_rfc3339(),
    }))
}

fn cmd_readback(cwd: &Path, command: &[String], artifacts: &[String]) -> BbxResult<Value> {
    if cwd.as_os_str().is_empty() || !cwd.exists() || !cwd.is_dir() {
        return Err(BlackboxError::connector_blocked(
            "CMD_WORKDIR_NOT_FOUND",
            CMD_CONNECTOR_ID,
            cmd_source_of_truth(cwd, command),
            "source.cwd",
            "command working directory does not exist",
            "create the working directory or pass the correct source.cwd path",
        ));
    }
    let output = run_process(cwd, command).map_err(|err| {
        BlackboxError::connector_blocked(
            "CMD_EXEC_FAILED",
            CMD_CONNECTOR_ID,
            cmd_source_of_truth(cwd, command),
            "expected.command",
            format!("failed to execute command argv: {err}"),
            "provide an executable command array; do not pass a shell string unless the shell is the explicit executable",
        )
    })?;
    let artifact_readbacks = artifacts
        .iter()
        .map(|artifact| {
            let path = cwd.join(artifact);
            if path.exists() && path.is_file() {
                let bytes = fs::read(&path).map_err(|err| {
                    BlackboxError::connector_blocked(
                        "CMD_ARTIFACT_READ_FAILED",
                        CMD_CONNECTOR_ID,
                        file_source_of_truth(&path),
                        "expected.artifacts",
                        format!("failed to read command artifact {artifact}: {err}"),
                        "fix artifact permissions and rerun verification",
                    )
                })?;
                Ok(json!({
                    "path": artifact,
                    "exists": true,
                    "size_bytes": bytes.len(),
                    "sha256": sha256_hex(&bytes),
                }))
            } else {
                Ok(json!({
                    "path": artifact,
                    "exists": false,
                    "size_bytes": null,
                    "sha256": null,
                }))
            }
        })
        .collect::<BbxResult<Vec<_>>>()?;
    Ok(json!({
        "cwd": cwd.display().to_string(),
        "command": command,
        "exit_code": output.exit_code,
        "stdout": output.stdout,
        "stderr": output.stderr,
        "artifacts": artifact_readbacks,
        "finished_at": Utc::now().to_rfc3339(),
    }))
}

fn run_git(repo: &Path, args: &[&str]) -> BbxResult<ProcessReadback> {
    let command = std::iter::once("git".to_string())
        .chain(std::iter::once("-C".to_string()))
        .chain(std::iter::once(repo.display().to_string()))
        .chain(args.iter().map(|arg| (*arg).to_string()))
        .collect::<Vec<_>>();
    run_process(Path::new("."), &command).map_err(|err| {
        BlackboxError::connector_blocked(
            "GIT_EXEC_FAILED",
            GIT_CONNECTOR_ID,
            git_source_of_truth(repo, "repository"),
            "git.exec",
            format!("failed to execute git argv {:?}: {err}", command),
            "install Git and ensure the git executable is available on PATH",
        )
    })
}

fn run_process(cwd: &Path, command: &[String]) -> std::io::Result<ProcessReadback> {
    if command.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "command argv must not be empty",
        ));
    }
    let mut cmd = Command::new(&command[0]);
    cmd.args(&command[1..]);
    if !cwd.as_os_str().is_empty() {
        cmd.current_dir(cwd);
    }
    let output = cmd.output()?;
    Ok(ProcessReadback {
        exit_code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

fn git_source_of_truth(repo: &Path, locator: &str) -> String {
    format!("git:{}#{locator}", repo.display())
}

fn file_source_of_truth(path: &Path) -> String {
    format!("file:{}#sha256", path.display())
}

fn cmd_source_of_truth(cwd: &Path, command: &[String]) -> String {
    let command_value = Value::Array(
        command
            .iter()
            .map(|item| Value::String(item.clone()))
            .collect(),
    );
    let digest = sha256_hex(
        &canonical_json_bytes(&command_value)
            .expect("command argv arrays of JSON strings serialize canonically"),
    );
    format!("cmd:{}#{digest}", cwd.display())
}

fn github_source_of_truth(owner: &str, repo: &str, locator: &str) -> String {
    github_source_of_truth_from_parts(owner, repo, locator)
}

fn github_source_of_truth_from_parts(owner: &str, repo: &str, locator: &str) -> String {
    format!("github:{owner}/{repo}#{locator}")
}

fn github_source_of_truth_from_locator(repo_locator: &str, locator: &str) -> String {
    format!("github:{repo_locator}#{locator}")
}

fn gha_source_of_truth(owner: &str, repo: &str, locator: &str) -> String {
    gha_source_of_truth_from_parts(owner, repo, locator)
}

fn gha_source_of_truth_from_parts(owner: &str, repo: &str, locator: &str) -> String {
    format!("gha:{owner}/{repo}#{locator}")
}

fn parse_github_repo_locator(locator: &str) -> BbxResult<(String, String)> {
    let trimmed = locator.trim().trim_matches('/');
    let parts = trimmed.split('/').collect::<Vec<_>>();
    if parts.len() != 2 || parts.iter().any(|part| part.trim().is_empty()) {
        return Err(BlackboxError::connector_blocked(
            "GITHUB_REPO_LOCATOR_INVALID",
            GITHUB_CONNECTOR_ID,
            github_source_of_truth_from_locator(locator, "repository"),
            "--source",
            "GitHub connector source must be OWNER/REPO",
            "pass a repository locator like actions/checkout",
        ));
    }
    Ok((parts[0].to_string(), parts[1].to_string()))
}

fn github_api_url(segments: &[&str], query: &[(&str, String)]) -> Url {
    let mut url = Url::parse(GITHUB_API_BASE).expect("GitHub API base URL is valid");
    {
        let mut path = url
            .path_segments_mut()
            .expect("GitHub API base URL supports path segments");
        for segment in segments {
            path.push(segment);
        }
    }
    if !query.is_empty() {
        let mut pairs = url.query_pairs_mut();
        for (key, value) in query {
            pairs.append_pair(key, value);
        }
    }
    url
}

fn github_token() -> Option<String> {
    std::env::var("BBX_GITHUB_TOKEN")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            std::env::var("GITHUB_TOKEN")
                .ok()
                .filter(|value| !value.trim().is_empty())
        })
}

fn github_client(source_of_truth: &str, connector: &'static str) -> BbxResult<Client> {
    Client::builder()
        .timeout(Duration::from_secs(20))
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .map_err(|err| {
            BlackboxError::connector_blocked(
                "GITHUB_HTTP_CLIENT_FAILED",
                connector,
                source_of_truth.to_string(),
                "github.client",
                format!("failed to build GitHub HTTP client: {err}"),
                "inspect the local TLS/network configuration and retry",
            )
        })
}

fn github_header(headers: &HeaderMap, name: &'static str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned)
}

fn github_rate_limit(headers: &HeaderMap) -> Value {
    json!({
        "limit": github_header(headers, "x-ratelimit-limit"),
        "remaining": github_header(headers, "x-ratelimit-remaining"),
        "used": github_header(headers, "x-ratelimit-used"),
        "reset": github_header(headers, "x-ratelimit-reset"),
        "resource": github_header(headers, "x-ratelimit-resource"),
        "retry_after": headers
            .get(RETRY_AFTER)
            .and_then(|value| value.to_str().ok())
            .map(ToOwned::to_owned),
    })
}

fn github_error_message(body: &Value) -> String {
    body.get("message")
        .and_then(Value::as_str)
        .unwrap_or("GitHub API returned an error")
        .to_string()
}

fn github_is_rate_limited(status: u16, headers: &HeaderMap, body: &Value) -> bool {
    if status == 429 {
        return true;
    }
    if headers.contains_key(RETRY_AFTER) {
        return true;
    }
    if github_header(headers, "x-ratelimit-remaining").as_deref() == Some("0") {
        return true;
    }
    github_error_message(body)
        .to_lowercase()
        .contains("rate limit")
}

fn github_allowed_non_success_response(
    status: u16,
    body: &Value,
    allowed_non_success_statuses: &[u16],
) -> bool {
    if !allowed_non_success_statuses.contains(&status) {
        return false;
    }
    if status != 422 {
        return true;
    }
    github_error_message(body).starts_with("No commit found for SHA:")
}

async fn github_get_json(
    connector: &'static str,
    url: Url,
    source_of_truth: &str,
    operation: &'static str,
    allowed_non_success_statuses: &[u16],
) -> BbxResult<GithubApiReadback> {
    let client = github_client(source_of_truth, connector)?;
    let mut request = client
        .get(url.clone())
        .header(USER_AGENT, HeaderValue::from_static(GITHUB_USER_AGENT))
        .header(
            ACCEPT,
            HeaderValue::from_static("application/vnd.github+json"),
        )
        .header("X-GitHub-Api-Version", GITHUB_API_VERSION);
    if let Some(token) = github_token() {
        let header_value = HeaderValue::from_str(&format!("Bearer {token}")).map_err(|err| {
            BlackboxError::connector_blocked(
                "CONNECTOR_CREDENTIAL_INVALID",
                connector,
                source_of_truth.to_string(),
                "env.BBX_GITHUB_TOKEN",
                format!("GitHub token could not be encoded as an HTTP header: {err}"),
                "replace BBX_GITHUB_TOKEN/GITHUB_TOKEN with a valid bearer token",
            )
        })?;
        request = request.header(AUTHORIZATION, header_value);
    }
    let response = request.send().await.map_err(|err| {
        BlackboxError::connector_blocked(
            "SOURCE_OF_TRUTH_UNAVAILABLE",
            connector,
            source_of_truth.to_string(),
            operation,
            format!("GitHub API request failed before an HTTP response was received: {err}"),
            "check network access, DNS/TLS, and GitHub availability, then retry",
        )
    })?;
    let status = response.status().as_u16();
    let headers = response.headers().clone();
    let url_after_redirects = response.url().to_string();
    let request_id = github_header(&headers, "x-github-request-id");
    let etag = github_header(&headers, "etag");
    let body_text = response.text().await.map_err(|err| {
        BlackboxError::connector_blocked(
            "GITHUB_RESPONSE_READ_FAILED",
            connector,
            source_of_truth.to_string(),
            operation,
            format!("failed to read GitHub API response body: {err}"),
            "retry after confirming the network connection is stable",
        )
    })?;
    let body = if body_text.trim().is_empty() {
        Value::Null
    } else {
        serde_json::from_str(&body_text).map_err(|err| {
            BlackboxError::connector_blocked(
                "READBACK_SCHEMA_DRIFT",
                connector,
                source_of_truth.to_string(),
                operation,
                format!("GitHub API response body was not valid JSON: {err}"),
                "inspect the response and update the connector only if GitHub changed the endpoint contract",
            )
        })?
    };
    let rate_limit = github_rate_limit(&headers);
    let body_sha256 = sha256_hex(body_text.as_bytes());
    if (200..300).contains(&status) {
        return Ok(GithubApiReadback {
            status,
            url: url_after_redirects,
            body,
            body_sha256,
            rate_limit,
            request_id,
            etag,
        });
    }
    if github_allowed_non_success_response(status, &body, allowed_non_success_statuses) {
        return Ok(GithubApiReadback {
            status,
            url: url_after_redirects,
            body,
            body_sha256,
            rate_limit,
            request_id,
            etag,
        });
    }
    if github_is_rate_limited(status, &headers, &body) {
        return Err(BlackboxError::connector_blocked(
            "GITHUB_RATE_LIMITED",
            connector,
            source_of_truth.to_string(),
            operation,
            format!(
                "GitHub API rate limit blocked readback: HTTP {status}: {}",
                github_error_message(&body)
            ),
            "wait until x-ratelimit-reset or retry-after, then rerun verification without claiming success",
        ));
    }
    if status == 401 || status == 403 {
        return Err(BlackboxError::connector_blocked(
            "CONNECTOR_PERMISSION_DENIED",
            connector,
            source_of_truth.to_string(),
            operation,
            format!(
                "GitHub API denied readback: HTTP {status}: {}",
                github_error_message(&body)
            ),
            "provide a token with the documented read permission or verify the repository is public",
        ));
    }
    if status == 409 || status >= 500 {
        return Err(BlackboxError::connector_blocked(
            "SOURCE_OF_TRUTH_UNAVAILABLE",
            connector,
            source_of_truth.to_string(),
            operation,
            format!(
                "GitHub source of truth could not provide a stable readback: HTTP {status}: {}",
                github_error_message(&body)
            ),
            "retry after the repository/API state is available; do not treat this as success",
        ));
    }
    Err(BlackboxError::connector_blocked(
        "GITHUB_HTTP_STATUS_UNEXPECTED",
        connector,
        source_of_truth.to_string(),
        operation,
        format!(
            "GitHub API returned unexpected HTTP {status}: {}",
            github_error_message(&body)
        ),
        "inspect the endpoint, request parameters, and GitHub API docs before retrying",
    ))
}

fn github_api_snapshot(response: &GithubApiReadback) -> Value {
    json!({
        "http_status": response.status,
        "url": response.url,
        "request_id": response.request_id,
        "etag": response.etag,
        "rate_limit": response.rate_limit,
        "body_sha256": response.body_sha256,
        "body": response.body,
    })
}

async fn github_repo_readback(owner: &str, repo: &str) -> BbxResult<Value> {
    let source = github_source_of_truth(owner, repo, "repository");
    let url = github_api_url(&["repos", owner, repo], &[]);
    let response = github_get_json(
        GITHUB_CONNECTOR_ID,
        url,
        &source,
        "GET /repos/{owner}/{repo}",
        &[404],
    )
    .await?;
    if response.status == 404 {
        return Err(BlackboxError::connector_blocked(
            "GITHUB_REPO_NOT_FOUND",
            GITHUB_CONNECTOR_ID,
            source,
            "source",
            "GitHub repository was not found",
            "verify OWNER/REPO and provide credentials for private repositories",
        ));
    }
    Ok(json!({
        "repository": github_repo_summary(&response.body),
        "api": github_api_snapshot(&response),
    }))
}

async fn gha_repo_readback(owner: &str, repo: &str) -> BbxResult<Value> {
    let source = gha_source_of_truth(owner, repo, "actions/runs");
    let url = github_api_url(
        &["repos", owner, repo, "actions", "runs"],
        &[("per_page", "1".to_string())],
    );
    let response = github_get_json(
        GHA_CONNECTOR_ID,
        url,
        &source,
        "GET /repos/{owner}/{repo}/actions/runs",
        &[404],
    )
    .await?;
    if response.status == 404 {
        return Err(BlackboxError::connector_blocked(
            "GHA_REPO_NOT_FOUND",
            GHA_CONNECTOR_ID,
            source,
            "source",
            "GitHub Actions repository was not found",
            "verify OWNER/REPO and provide Actions read credentials for private repositories",
        ));
    }
    Ok(json!({
        "total_count": response.body.get("total_count").cloned().unwrap_or(Value::Null),
        "latest_run": response.body
            .get("workflow_runs")
            .and_then(Value::as_array)
            .and_then(|runs| runs.first())
            .map(github_workflow_run_summary),
        "api": github_api_snapshot(&response),
    }))
}

fn github_repo_summary(body: &Value) -> Value {
    json!({
        "id": body.get("id").cloned().unwrap_or(Value::Null),
        "full_name": body.get("full_name").cloned().unwrap_or(Value::Null),
        "private": body.get("private").cloned().unwrap_or(Value::Null),
        "default_branch": body.get("default_branch").cloned().unwrap_or(Value::Null),
        "html_url": body.get("html_url").cloned().unwrap_or(Value::Null),
        "pushed_at": body.get("pushed_at").cloned().unwrap_or(Value::Null),
        "updated_at": body.get("updated_at").cloned().unwrap_or(Value::Null),
    })
}

fn github_pr_summary(body: &Value) -> Value {
    json!({
        "number": body.get("number").cloned().unwrap_or(Value::Null),
        "state": body.get("state").cloned().unwrap_or(Value::Null),
        "draft": body.get("draft").cloned().unwrap_or(Value::Null),
        "merged": body.get("merged").cloned().unwrap_or(Value::Null),
        "head_ref": body.pointer("/head/ref").cloned().unwrap_or(Value::Null),
        "head_sha": body.pointer("/head/sha").cloned().unwrap_or(Value::Null),
        "base_ref": body.pointer("/base/ref").cloned().unwrap_or(Value::Null),
        "base_sha": body.pointer("/base/sha").cloned().unwrap_or(Value::Null),
        "html_url": body.get("html_url").cloned().unwrap_or(Value::Null),
        "updated_at": body.get("updated_at").cloned().unwrap_or(Value::Null),
    })
}

fn github_issue_summary(body: &Value) -> Value {
    json!({
        "number": body.get("number").cloned().unwrap_or(Value::Null),
        "state": body.get("state").cloned().unwrap_or(Value::Null),
        "state_reason": body.get("state_reason").cloned().unwrap_or(Value::Null),
        "title": body.get("title").cloned().unwrap_or(Value::Null),
        "html_url": body.get("html_url").cloned().unwrap_or(Value::Null),
        "updated_at": body.get("updated_at").cloned().unwrap_or(Value::Null),
        "is_pull_request": body.get("pull_request").is_some(),
    })
}

fn github_commit_summary(body: &Value) -> Value {
    json!({
        "sha": body.get("sha").cloned().unwrap_or(Value::Null),
        "html_url": body.get("html_url").cloned().unwrap_or(Value::Null),
        "message": body.pointer("/commit/message").cloned().unwrap_or(Value::Null),
        "author_date": body.pointer("/commit/author/date").cloned().unwrap_or(Value::Null),
        "committer_date": body.pointer("/commit/committer/date").cloned().unwrap_or(Value::Null),
    })
}

fn github_check_suite_summary(body: &Value) -> Value {
    json!({
        "id": body.get("id").cloned().unwrap_or(Value::Null),
        "status": body.get("status").cloned().unwrap_or(Value::Null),
        "conclusion": body.get("conclusion").cloned().unwrap_or(Value::Null),
        "head_sha": body.get("head_sha").cloned().unwrap_or(Value::Null),
        "app_slug": body.pointer("/app/slug").cloned().unwrap_or(Value::Null),
        "check_runs_url": body.get("check_runs_url").cloned().unwrap_or(Value::Null),
        "updated_at": body.get("updated_at").cloned().unwrap_or(Value::Null),
    })
}

fn github_workflow_run_summary(body: &Value) -> Value {
    json!({
        "id": body.get("id").cloned().unwrap_or(Value::Null),
        "name": body.get("name").cloned().unwrap_or(Value::Null),
        "status": body.get("status").cloned().unwrap_or(Value::Null),
        "conclusion": body.get("conclusion").cloned().unwrap_or(Value::Null),
        "head_sha": body.get("head_sha").cloned().unwrap_or(Value::Null),
        "head_branch": body.get("head_branch").cloned().unwrap_or(Value::Null),
        "event": body.get("event").cloned().unwrap_or(Value::Null),
        "workflow_id": body.get("workflow_id").cloned().unwrap_or(Value::Null),
        "html_url": body.get("html_url").cloned().unwrap_or(Value::Null),
        "updated_at": body.get("updated_at").cloned().unwrap_or(Value::Null),
    })
}

fn github_owner_repo_from_spec(
    spec: &VerifySpec,
    connector: &'static str,
    source_hint: &str,
) -> Result<(String, String), VerificationOutcome> {
    let owner = required_source_string(spec, "owner", "$.source.owner", connector, source_hint)?;
    let repo = required_source_string(spec, "repo", "$.source.repo", connector, source_hint)?;
    Ok((owner, repo))
}

fn github_readback_payload(
    verifier_id: &str,
    connector: &'static str,
    source: &str,
    operation: &str,
    response: &GithubApiReadback,
    actual: &Value,
) -> Value {
    readback_payload(
        "readback",
        verifier_id,
        connector,
        source,
        operation,
        json!({
            "api": github_api_snapshot(response),
            "observed": actual,
        }),
    )
}

async fn verify_github_pr_state(spec: &VerifySpec) -> VerificationOutcome {
    let source_hint = github_source_of_truth_from_parts(
        optional_str(&spec.source, "owner").unwrap_or(""),
        optional_str(&spec.source, "repo").unwrap_or(""),
        "pulls/unknown",
    );
    let (owner, repo) = match github_owner_repo_from_spec(spec, GITHUB_CONNECTOR_ID, &source_hint) {
        Ok(parts) => parts,
        Err(outcome) => return outcome,
    };
    let pr_number = match required_expected_u64(
        spec,
        "pr_number",
        "$.expected.pr_number",
        GITHUB_CONNECTOR_ID,
        &source_hint,
    ) {
        Ok(number) => number,
        Err(outcome) => return outcome,
    };
    let expected_state = match required_expected_string(
        spec,
        "state",
        "$.expected.state",
        GITHUB_CONNECTOR_ID,
        &source_hint,
        false,
    ) {
        Ok(state) => state,
        Err(outcome) => return outcome,
    };
    let source = github_source_of_truth(&owner, &repo, &format!("pulls/{pr_number}"));
    let url = github_api_url(
        &["repos", &owner, &repo, "pulls", &pr_number.to_string()],
        &[],
    );
    let response = match github_get_json(
        GITHUB_CONNECTOR_ID,
        url,
        &source,
        "GET /repos/{owner}/{repo}/pulls/{pull_number}",
        &[404],
    )
    .await
    {
        Ok(response) => response,
        Err(err) => return connector_error_to_outcome(err),
    };
    let actual = if response.status == 404 {
        json!({
            "pr_number": pr_number,
            "exists": false,
            "http_status": response.status,
            "message": github_error_message(&response.body),
        })
    } else {
        let summary = github_pr_summary(&response.body);
        json!({
            "pr_number": pr_number,
            "exists": true,
            "http_status": response.status,
            "state": summary.get("state").cloned().unwrap_or(Value::Null),
            "head_sha": summary.get("head_sha").cloned().unwrap_or(Value::Null),
            "head_ref": summary.get("head_ref").cloned().unwrap_or(Value::Null),
            "base_ref": summary.get("base_ref").cloned().unwrap_or(Value::Null),
            "draft": summary.get("draft").cloned().unwrap_or(Value::Null),
            "merged": summary.get("merged").cloned().unwrap_or(Value::Null),
            "html_url": summary.get("html_url").cloned().unwrap_or(Value::Null),
            "updated_at": summary.get("updated_at").cloned().unwrap_or(Value::Null),
        })
    };
    let readback = github_readback_payload(
        "github_pr_state_v1",
        GITHUB_CONNECTOR_ID,
        &source,
        "GET /repos/{owner}/{repo}/pulls/{pull_number}",
        &response,
        &actual,
    );
    if response.status == 404 {
        return VerificationOutcome::failed(
            GITHUB_CONNECTOR_ID,
            source,
            "GITHUB_PR_NOT_FOUND",
            "actual_readback.exists",
            "expected pull request does not exist in GitHub",
            "verify the pull request number or create the pull request before claiming success",
            readback,
            actual,
        );
    }
    if actual.get("state").and_then(Value::as_str) == Some(expected_state.as_str()) {
        VerificationOutcome::passed(GITHUB_CONNECTOR_ID, source, readback, actual)
    } else {
        VerificationOutcome::failed(
            GITHUB_CONNECTOR_ID,
            source,
            "GITHUB_PR_STATE_MISMATCH",
            "actual_readback.state",
            "GitHub pull request state did not match the expected state",
            "read the GitHub pull request and update the expected delta or fix the PR state",
            readback,
            actual,
        )
    }
}

async fn verify_github_branch_head_matches_commit(spec: &VerifySpec) -> VerificationOutcome {
    let source_hint = github_source_of_truth_from_parts(
        optional_str(&spec.source, "owner").unwrap_or(""),
        optional_str(&spec.source, "repo").unwrap_or(""),
        "heads/unknown",
    );
    let (owner, repo) = match github_owner_repo_from_spec(spec, GITHUB_CONNECTOR_ID, &source_hint) {
        Ok(parts) => parts,
        Err(outcome) => return outcome,
    };
    let branch = match required_expected_string(
        spec,
        "branch",
        "$.expected.branch",
        GITHUB_CONNECTOR_ID,
        &source_hint,
        false,
    ) {
        Ok(branch) => branch,
        Err(outcome) => return outcome,
    };
    let expected_commit = match required_expected_string(
        spec,
        "commit",
        "$.expected.commit",
        GITHUB_CONNECTOR_ID,
        &source_hint,
        false,
    ) {
        Ok(commit) => commit,
        Err(outcome) => return outcome,
    };
    let source = github_source_of_truth(&owner, &repo, &format!("heads/{branch}"));
    let ref_name = format!("heads/{branch}");
    let url = github_api_url(&["repos", &owner, &repo, "commits", &ref_name], &[]);
    let response = match github_get_json(
        GITHUB_CONNECTOR_ID,
        url,
        &source,
        "GET /repos/{owner}/{repo}/commits/{ref}",
        &[404, 422],
    )
    .await
    {
        Ok(response) => response,
        Err(err) => return connector_error_to_outcome(err),
    };
    let missing_branch = response.status == 404 || response.status == 422;
    let actual = if missing_branch {
        json!({
            "branch": branch,
            "exists": false,
            "http_status": response.status,
            "message": github_error_message(&response.body),
        })
    } else {
        let summary = github_commit_summary(&response.body);
        json!({
            "branch": branch,
            "exists": true,
            "http_status": response.status,
            "sha": summary.get("sha").cloned().unwrap_or(Value::Null),
            "html_url": summary.get("html_url").cloned().unwrap_or(Value::Null),
            "committer_date": summary.get("committer_date").cloned().unwrap_or(Value::Null),
        })
    };
    let readback = github_readback_payload(
        "github_branch_head_matches_commit_v1",
        GITHUB_CONNECTOR_ID,
        &source,
        "GET /repos/{owner}/{repo}/commits/{ref}",
        &response,
        &actual,
    );
    if missing_branch {
        return VerificationOutcome::failed(
            GITHUB_CONNECTOR_ID,
            source,
            "GITHUB_BRANCH_NOT_FOUND",
            "actual_readback.exists",
            "expected GitHub branch/ref does not exist",
            "verify the branch name or push the branch before claiming success",
            readback,
            actual,
        );
    }
    if actual.get("sha").and_then(Value::as_str) == Some(expected_commit.as_str()) {
        VerificationOutcome::passed(GITHUB_CONNECTOR_ID, source, readback, actual)
    } else {
        VerificationOutcome::failed(
            GITHUB_CONNECTOR_ID,
            source,
            "GITHUB_BRANCH_HEAD_MISMATCH",
            "actual_readback.sha",
            "GitHub branch head SHA did not match the expected commit",
            "push the expected commit to the branch or update the expected delta",
            readback,
            actual,
        )
    }
}

async fn verify_github_issue_state_changed(spec: &VerifySpec) -> VerificationOutcome {
    let source_hint = github_source_of_truth_from_parts(
        optional_str(&spec.source, "owner").unwrap_or(""),
        optional_str(&spec.source, "repo").unwrap_or(""),
        "issues/unknown",
    );
    let (owner, repo) = match github_owner_repo_from_spec(spec, GITHUB_CONNECTOR_ID, &source_hint) {
        Ok(parts) => parts,
        Err(outcome) => return outcome,
    };
    let issue_number = match required_expected_u64(
        spec,
        "issue_number",
        "$.expected.issue_number",
        GITHUB_CONNECTOR_ID,
        &source_hint,
    ) {
        Ok(number) => number,
        Err(outcome) => return outcome,
    };
    let expected_state = match required_expected_string(
        spec,
        "state",
        "$.expected.state",
        GITHUB_CONNECTOR_ID,
        &source_hint,
        false,
    ) {
        Ok(state) => state,
        Err(outcome) => return outcome,
    };
    let source = github_source_of_truth(&owner, &repo, &format!("issues/{issue_number}"));
    let url = github_api_url(
        &["repos", &owner, &repo, "issues", &issue_number.to_string()],
        &[],
    );
    let response = match github_get_json(
        GITHUB_CONNECTOR_ID,
        url,
        &source,
        "GET /repos/{owner}/{repo}/issues/{issue_number}",
        &[404],
    )
    .await
    {
        Ok(response) => response,
        Err(err) => return connector_error_to_outcome(err),
    };
    let actual = if response.status == 404 {
        json!({
            "issue_number": issue_number,
            "exists": false,
            "http_status": response.status,
            "message": github_error_message(&response.body),
        })
    } else {
        let summary = github_issue_summary(&response.body);
        json!({
            "issue_number": issue_number,
            "exists": true,
            "http_status": response.status,
            "state": summary.get("state").cloned().unwrap_or(Value::Null),
            "state_reason": summary.get("state_reason").cloned().unwrap_or(Value::Null),
            "is_pull_request": summary.get("is_pull_request").cloned().unwrap_or(Value::Bool(false)),
            "title": summary.get("title").cloned().unwrap_or(Value::Null),
            "html_url": summary.get("html_url").cloned().unwrap_or(Value::Null),
            "updated_at": summary.get("updated_at").cloned().unwrap_or(Value::Null),
        })
    };
    let readback = github_readback_payload(
        "github_issue_state_changed_v1",
        GITHUB_CONNECTOR_ID,
        &source,
        "GET /repos/{owner}/{repo}/issues/{issue_number}",
        &response,
        &actual,
    );
    if response.status == 404 {
        return VerificationOutcome::failed(
            GITHUB_CONNECTOR_ID,
            source,
            "GITHUB_ISSUE_NOT_FOUND",
            "actual_readback.exists",
            "expected GitHub issue does not exist",
            "verify the issue number or create the issue before claiming success",
            readback,
            actual,
        );
    }
    if actual
        .get("is_pull_request")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return VerificationOutcome::failed(
            GITHUB_CONNECTOR_ID,
            source,
            "GITHUB_ISSUE_IS_PULL_REQUEST",
            "actual_readback.is_pull_request",
            "GitHub issue endpoint returned a pull request, not an issue",
            "use github_pr_state_v1 for pull requests or provide a real issue number",
            readback,
            actual,
        );
    }
    if actual.get("state").and_then(Value::as_str) == Some(expected_state.as_str()) {
        VerificationOutcome::passed(GITHUB_CONNECTOR_ID, source, readback, actual)
    } else {
        VerificationOutcome::failed(
            GITHUB_CONNECTOR_ID,
            source,
            "GITHUB_ISSUE_STATE_MISMATCH",
            "actual_readback.state",
            "GitHub issue state did not match the expected state",
            "read the GitHub issue and update the expected delta or fix the issue state",
            readback,
            actual,
        )
    }
}

async fn verify_github_check_suite_passed(spec: &VerifySpec) -> VerificationOutcome {
    let source_hint = github_source_of_truth_from_parts(
        optional_str(&spec.source, "owner").unwrap_or(""),
        optional_str(&spec.source, "repo").unwrap_or(""),
        "commits/unknown/check-suites",
    );
    let (owner, repo) = match github_owner_repo_from_spec(spec, GITHUB_CONNECTOR_ID, &source_hint) {
        Ok(parts) => parts,
        Err(outcome) => return outcome,
    };
    let git_ref = match required_expected_string(
        spec,
        "ref",
        "$.expected.ref",
        GITHUB_CONNECTOR_ID,
        &source_hint,
        false,
    ) {
        Ok(git_ref) => git_ref,
        Err(outcome) => return outcome,
    };
    let expected_conclusion = match required_expected_string(
        spec,
        "conclusion",
        "$.expected.conclusion",
        GITHUB_CONNECTOR_ID,
        &source_hint,
        false,
    ) {
        Ok(conclusion) => conclusion,
        Err(outcome) => return outcome,
    };
    let source = github_source_of_truth(&owner, &repo, &format!("commits/{git_ref}/check-suites"));
    let url = github_api_url(
        &["repos", &owner, &repo, "commits", &git_ref, "check-suites"],
        &[("per_page", "100".to_string())],
    );
    let response = match github_get_json(
        GITHUB_CONNECTOR_ID,
        url,
        &source,
        "GET /repos/{owner}/{repo}/commits/{ref}/check-suites",
        &[404, 422],
    )
    .await
    {
        Ok(response) => response,
        Err(err) => return connector_error_to_outcome(err),
    };
    let suites = response
        .body
        .get("check_suites")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let suite_summaries = suites
        .iter()
        .map(github_check_suite_summary)
        .collect::<Vec<_>>();
    let total_count = response
        .body
        .get("total_count")
        .and_then(Value::as_u64)
        .unwrap_or(suite_summaries.len() as u64);
    let mismatched = suite_summaries
        .iter()
        .filter(|suite| {
            suite.get("conclusion").and_then(Value::as_str) != Some(expected_conclusion.as_str())
        })
        .count();
    let actual = json!({
        "ref": git_ref,
        "http_status": response.status,
        "total_count": total_count,
        "expected_conclusion": expected_conclusion,
        "mismatched_count": mismatched,
        "check_suites": suite_summaries,
    });
    let readback = github_readback_payload(
        "github_check_suite_passed_v1",
        GITHUB_CONNECTOR_ID,
        &source,
        "GET /repos/{owner}/{repo}/commits/{ref}/check-suites",
        &response,
        &actual,
    );
    if response.status == 404 || total_count == 0 {
        return VerificationOutcome::failed(
            GITHUB_CONNECTOR_ID,
            source,
            "GITHUB_CHECK_SUITE_NOT_FOUND",
            "actual_readback.total_count",
            "no GitHub check suites were found for the expected ref",
            "wait for checks to be created or verify the expected ref",
            readback,
            actual,
        );
    }
    if mismatched == 0 {
        VerificationOutcome::passed(GITHUB_CONNECTOR_ID, source, readback, actual)
    } else {
        VerificationOutcome::failed(
            GITHUB_CONNECTOR_ID,
            source,
            "GITHUB_CHECK_SUITE_CONCLUSION_MISMATCH",
            "actual_readback.check_suites",
            "one or more GitHub check suites did not match the expected conclusion",
            "inspect the failing/pending check suites before claiming CI passed",
            readback,
            actual,
        )
    }
}

async fn verify_gha_workflow_run_completed(spec: &VerifySpec) -> VerificationOutcome {
    let source_hint = gha_source_of_truth_from_parts(
        optional_str(&spec.source, "owner").unwrap_or(""),
        optional_str(&spec.source, "repo").unwrap_or(""),
        "actions/runs/unknown",
    );
    let (owner, repo) = match github_owner_repo_from_spec(spec, GHA_CONNECTOR_ID, &source_hint) {
        Ok(parts) => parts,
        Err(outcome) => return outcome,
    };
    let run_id = match required_expected_u64(
        spec,
        "run_id",
        "$.expected.run_id",
        GHA_CONNECTOR_ID,
        &source_hint,
    ) {
        Ok(run_id) => run_id,
        Err(outcome) => return outcome,
    };
    let expected_status = match required_expected_string(
        spec,
        "status",
        "$.expected.status",
        GHA_CONNECTOR_ID,
        &source_hint,
        false,
    ) {
        Ok(status) => status,
        Err(outcome) => return outcome,
    };
    let expected_conclusion = match required_expected_string(
        spec,
        "conclusion",
        "$.expected.conclusion",
        GHA_CONNECTOR_ID,
        &source_hint,
        false,
    ) {
        Ok(conclusion) => conclusion,
        Err(outcome) => return outcome,
    };
    let source = gha_source_of_truth(&owner, &repo, &format!("actions/runs/{run_id}"));
    let url = github_api_url(
        &[
            "repos",
            &owner,
            &repo,
            "actions",
            "runs",
            &run_id.to_string(),
        ],
        &[],
    );
    let response = match github_get_json(
        GHA_CONNECTOR_ID,
        url,
        &source,
        "GET /repos/{owner}/{repo}/actions/runs/{run_id}",
        &[404],
    )
    .await
    {
        Ok(response) => response,
        Err(err) => return connector_error_to_outcome(err),
    };
    let actual = if response.status == 404 {
        json!({
            "run_id": run_id,
            "exists": false,
            "http_status": response.status,
            "message": github_error_message(&response.body),
        })
    } else {
        let summary = github_workflow_run_summary(&response.body);
        json!({
            "run_id": run_id,
            "exists": true,
            "http_status": response.status,
            "status": summary.get("status").cloned().unwrap_or(Value::Null),
            "conclusion": summary.get("conclusion").cloned().unwrap_or(Value::Null),
            "head_sha": summary.get("head_sha").cloned().unwrap_or(Value::Null),
            "head_branch": summary.get("head_branch").cloned().unwrap_or(Value::Null),
            "event": summary.get("event").cloned().unwrap_or(Value::Null),
            "workflow_id": summary.get("workflow_id").cloned().unwrap_or(Value::Null),
            "html_url": summary.get("html_url").cloned().unwrap_or(Value::Null),
            "updated_at": summary.get("updated_at").cloned().unwrap_or(Value::Null),
        })
    };
    let readback = github_readback_payload(
        "gha_workflow_run_completed_v1",
        GHA_CONNECTOR_ID,
        &source,
        "GET /repos/{owner}/{repo}/actions/runs/{run_id}",
        &response,
        &actual,
    );
    if response.status == 404 {
        return VerificationOutcome::failed(
            GHA_CONNECTOR_ID,
            source,
            "GHA_WORKFLOW_RUN_NOT_FOUND",
            "actual_readback.exists",
            "expected GitHub Actions workflow run does not exist",
            "verify the workflow run id before claiming CI completed",
            readback,
            actual,
        );
    }
    if actual.get("status").and_then(Value::as_str) != Some(expected_status.as_str()) {
        return VerificationOutcome::failed(
            GHA_CONNECTOR_ID,
            source,
            "GHA_WORKFLOW_RUN_STATUS_MISMATCH",
            "actual_readback.status",
            "GitHub Actions workflow run status did not match the expected status",
            "wait for the workflow run to reach the expected status or update the expected delta",
            readback,
            actual,
        );
    }
    if actual.get("conclusion").and_then(Value::as_str) == Some(expected_conclusion.as_str()) {
        VerificationOutcome::passed(GHA_CONNECTOR_ID, source, readback, actual)
    } else {
        VerificationOutcome::failed(
            GHA_CONNECTOR_ID,
            source,
            "GHA_WORKFLOW_RUN_CONCLUSION_MISMATCH",
            "actual_readback.conclusion",
            "GitHub Actions workflow run conclusion did not match the expected conclusion",
            "inspect the workflow run before claiming CI passed",
            readback,
            actual,
        )
    }
}

fn verify_git_branch_exists(spec: &VerifySpec) -> VerificationOutcome {
    let repo = PathBuf::from(optional_str(&spec.source, "repo").unwrap_or(""));
    let branch_hint = optional_str(&spec.expected, "branch").unwrap_or("");
    let source = git_source_of_truth(&repo, &format!("refs/heads/{branch_hint}"));
    let branch = match required_expected_string(
        spec,
        "branch",
        "$.expected.branch",
        GIT_CONNECTOR_ID,
        &source,
        false,
    ) {
        Ok(branch) => branch,
        Err(outcome) => return outcome,
    };
    let repo_readback = match git_repo_readback(&repo) {
        Ok(readback) => readback,
        Err(err) => return connector_error_to_outcome(err),
    };
    let rev = format!("refs/heads/{branch}^{{commit}}");
    let output = match run_git(
        &repo,
        &["rev-parse", "--verify", "--quiet", "--end-of-options", &rev],
    ) {
        Ok(output) => output,
        Err(err) => return connector_error_to_outcome(err),
    };
    let exists = output.exit_code == 0;
    let commit = if exists {
        Some(output.stdout.trim().to_string())
    } else {
        None
    };
    let actual = json!({
        "branch": branch,
        "branch_exists": exists,
        "commit": commit,
        "git_exit_code": output.exit_code,
        "git_stdout": output.stdout,
        "git_stderr": output.stderr,
    });
    let readback = readback_payload(
        "readback",
        "git_branch_exists_v1",
        GIT_CONNECTOR_ID,
        &source,
        "rev-parse --verify --quiet refs/heads/<branch>^{commit}",
        json!({
            "repository": repo_readback,
            "observed": actual,
        }),
    );
    if exists {
        VerificationOutcome::passed(GIT_CONNECTOR_ID, source, readback, actual)
    } else {
        VerificationOutcome::failed(
            GIT_CONNECTOR_ID,
            source,
            "GIT_BRANCH_NOT_FOUND",
            "actual_readback.branch_exists",
            "expected branch does not exist in the Git source of truth",
            "create the branch or verify the expected branch name",
            readback,
            actual,
        )
    }
}

fn verify_git_commit_exists(spec: &VerifySpec) -> VerificationOutcome {
    let repo = PathBuf::from(optional_str(&spec.source, "repo").unwrap_or(""));
    let commit_hint = optional_str(&spec.expected, "commit").unwrap_or("");
    let source = git_source_of_truth(&repo, commit_hint);
    let commit = match required_expected_string(
        spec,
        "commit",
        "$.expected.commit",
        GIT_CONNECTOR_ID,
        &source,
        false,
    ) {
        Ok(commit) => commit,
        Err(outcome) => return outcome,
    };
    let repo_readback = match git_repo_readback(&repo) {
        Ok(readback) => readback,
        Err(err) => return connector_error_to_outcome(err),
    };
    let rev = format!("{commit}^{{commit}}");
    let output = match run_git(
        &repo,
        &["rev-parse", "--verify", "--quiet", "--end-of-options", &rev],
    ) {
        Ok(output) => output,
        Err(err) => return connector_error_to_outcome(err),
    };
    let object_type_output = match run_git(&repo, &["cat-file", "-t", "--", &commit]) {
        Ok(output) => output,
        Err(err) => return connector_error_to_outcome(err),
    };
    let exists = output.exit_code == 0;
    let resolved_commit = if exists {
        Some(output.stdout.trim().to_string())
    } else {
        None
    };
    let object_type = if object_type_output.exit_code == 0 {
        Some(object_type_output.stdout.trim().to_string())
    } else {
        None
    };
    let actual = json!({
        "commit": commit,
        "exists": exists,
        "commit_exists": exists,
        "resolved_commit": resolved_commit,
        "object_type": object_type,
        "git_exit_code": output.exit_code,
        "git_stdout": output.stdout,
        "git_stderr": output.stderr,
        "git_object_type_exit_code": object_type_output.exit_code,
        "git_object_type_stdout": object_type_output.stdout,
        "git_object_type_stderr": object_type_output.stderr,
    });
    let readback = readback_payload(
        "readback",
        "git_commit_exists_v1",
        GIT_CONNECTOR_ID,
        &source,
        "rev-parse --verify --quiet <commit>^{commit}; cat-file -t <commit>",
        json!({
            "repository": repo_readback,
            "observed": actual,
        }),
    );
    if exists {
        VerificationOutcome::passed(GIT_CONNECTOR_ID, source, readback, actual)
    } else {
        VerificationOutcome::failed(
            GIT_CONNECTOR_ID,
            source,
            "GIT_COMMIT_NOT_FOUND",
            "actual_readback.commit_exists",
            "expected commit does not exist as a commit object in the Git source of truth",
            "use a commit object id that exists in the repository",
            readback,
            actual,
        )
    }
}

fn verify_git_working_tree_clean(spec: &VerifySpec) -> VerificationOutcome {
    let repo = PathBuf::from(optional_str(&spec.source, "repo").unwrap_or(""));
    let source = git_source_of_truth(&repo, "working_tree");
    let expected_clean = match required_expected_bool(
        spec,
        "clean",
        "$.expected.clean",
        GIT_CONNECTOR_ID,
        &source,
    ) {
        Ok(expected_clean) => expected_clean,
        Err(outcome) => return outcome,
    };
    let repo_readback = match git_repo_readback(&repo) {
        Ok(readback) => readback,
        Err(err) => return connector_error_to_outcome(err),
    };
    let output = match run_git(
        &repo,
        &["status", "--porcelain=v1", "--untracked-files=all"],
    ) {
        Ok(output) => output,
        Err(err) => return connector_error_to_outcome(err),
    };
    if output.exit_code != 0 {
        return VerificationOutcome::blocked(
            GIT_CONNECTOR_ID,
            source,
            "GIT_STATUS_FAILED",
            "git.status",
            "git status failed for the repository",
            "inspect git stderr, repository permissions, and repository health",
            Some(readback_payload(
                "readback",
                "git_working_tree_clean_v1",
                GIT_CONNECTOR_ID,
                &git_source_of_truth(&repo, "working_tree"),
                "status --porcelain=v1 --untracked-files=all",
                json!({ "git_exit_code": output.exit_code, "git_stdout": output.stdout, "git_stderr": output.stderr }),
            )),
        );
    }
    let clean = output.stdout.trim().is_empty();
    let actual = json!({
        "clean": clean,
        "status_porcelain_v1": output.stdout,
    });
    let readback = readback_payload(
        "readback",
        "git_working_tree_clean_v1",
        GIT_CONNECTOR_ID,
        &source,
        "status --porcelain=v1 --untracked-files=all",
        json!({
            "repository": repo_readback,
            "observed": actual,
        }),
    );
    if clean == expected_clean {
        VerificationOutcome::passed(GIT_CONNECTOR_ID, source, readback, actual)
    } else {
        VerificationOutcome::failed(
            GIT_CONNECTOR_ID,
            source,
            "GIT_WORKING_TREE_DIRTY",
            "actual_readback.clean",
            "Git working tree cleanliness did not match the expected state",
            "commit, revert, or remove working tree changes before claiming the run is clean",
            readback,
            actual,
        )
    }
}

fn verify_git_file_diff_matches_patch(spec: &VerifySpec) -> VerificationOutcome {
    let repo = PathBuf::from(optional_str(&spec.source, "repo").unwrap_or(""));
    let path_hint = optional_str(&spec.expected, "path").unwrap_or("");
    let source = git_source_of_truth(&repo, path_hint);
    let path = match required_expected_string(
        spec,
        "path",
        "$.expected.path",
        GIT_CONNECTOR_ID,
        &source,
        false,
    ) {
        Ok(path) => path,
        Err(outcome) => return outcome,
    };
    let expected_patch = match required_expected_string(
        spec,
        "patch",
        "$.expected.patch",
        GIT_CONNECTOR_ID,
        &source,
        true,
    ) {
        Ok(patch) => patch,
        Err(outcome) => return outcome,
    };
    let repo_readback = match git_repo_readback(&repo) {
        Ok(readback) => readback,
        Err(err) => return connector_error_to_outcome(err),
    };
    let output = match run_git(&repo, &["diff", "--no-ext-diff", "--no-color", "--", &path]) {
        Ok(output) => output,
        Err(err) => return connector_error_to_outcome(err),
    };
    if output.exit_code != 0 {
        return VerificationOutcome::blocked(
            GIT_CONNECTOR_ID,
            source,
            "GIT_DIFF_FAILED",
            "git.diff",
            "git diff failed for the requested path",
            "inspect git stderr and verify the pathspec is valid for this repository",
            Some(readback_payload(
                "readback",
                "git_file_diff_matches_patch_v1",
                GIT_CONNECTOR_ID,
                &git_source_of_truth(&repo, &path),
                "diff --no-ext-diff --no-color -- <path>",
                json!({ "git_exit_code": output.exit_code, "git_stdout": output.stdout, "git_stderr": output.stderr }),
            )),
        );
    }
    let actual = json!({
        "path": path,
        "patch": output.stdout,
        "patch_sha256": sha256_hex(output.stdout.as_bytes()),
        "expected_patch_sha256": sha256_hex(expected_patch.as_bytes()),
    });
    let readback = readback_payload(
        "readback",
        "git_file_diff_matches_patch_v1",
        GIT_CONNECTOR_ID,
        &source,
        "diff --no-ext-diff --no-color -- <path>",
        json!({
            "repository": repo_readback,
            "observed": actual,
        }),
    );
    if output.stdout == expected_patch {
        VerificationOutcome::passed(GIT_CONNECTOR_ID, source, readback, actual)
    } else {
        VerificationOutcome::failed(
            GIT_CONNECTOR_ID,
            source,
            "GIT_FILE_DIFF_MISMATCH",
            "actual_readback.patch",
            "Git diff readback does not match the expected patch exactly",
            "read the physical git diff and update the expected delta or fix the working tree",
            readback,
            actual,
        )
    }
}

fn verify_cmd_exit_zero(spec: &VerifySpec) -> VerificationOutcome {
    let cwd = PathBuf::from(optional_str(&spec.source, "cwd").unwrap_or(""));
    let command_hint = optional_string_array(&spec.expected, "command").unwrap_or_default();
    let mut source = cmd_source_of_truth(&cwd, &command_hint);
    let command = match expected_string_array(
        spec,
        "command",
        "$.expected.command",
        CMD_CONNECTOR_ID,
        &source,
        true,
    ) {
        Ok(command) => command,
        Err(outcome) => return outcome,
    };
    source = cmd_source_of_truth(&cwd, &command);
    let artifacts = match expected_string_array(
        spec,
        "artifacts",
        "$.expected.artifacts",
        CMD_CONNECTOR_ID,
        &source,
        false,
    ) {
        Ok(artifacts) => artifacts,
        Err(outcome) => return outcome,
    };
    match cmd_readback(&cwd, &command, &artifacts) {
        Ok(readback) => {
            let exit_code = readback
                .get("exit_code")
                .and_then(Value::as_i64)
                .unwrap_or(-1);
            let missing_artifacts = readback
                .get("artifacts")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter(|item| {
                            !item.get("exists").and_then(Value::as_bool).unwrap_or(false)
                        })
                        .count()
                })
                .unwrap_or(0);
            let payload = readback_payload(
                "readback",
                "cmd_exit_zero_v1",
                CMD_CONNECTOR_ID,
                &source,
                "std::process::Command argv execution",
                readback.clone(),
            );
            if missing_artifacts > 0 {
                VerificationOutcome::failed(
                    CMD_CONNECTOR_ID,
                    source,
                    "CMD_ARTIFACT_MISSING",
                    "actual_readback.artifacts",
                    "one or more declared command artifacts do not exist after command execution",
                    "inspect the command output and write the declared artifact before claiming success",
                    payload,
                    readback,
                )
            } else if exit_code == 0 {
                VerificationOutcome::passed(CMD_CONNECTOR_ID, source, payload, readback)
            } else {
                VerificationOutcome::failed(
                    CMD_CONNECTOR_ID,
                    source,
                    "CMD_EXIT_NON_ZERO",
                    "actual_readback.exit_code",
                    "command exit code was non-zero",
                    "inspect stdout/stderr and rerun only after the command exits zero",
                    payload,
                    readback,
                )
            }
        }
        Err(err) => connector_error_to_outcome(err),
    }
}

fn connector_error_to_outcome(err: BlackboxError) -> VerificationOutcome {
    let field_path = err.field_path.unwrap_or_else(|| "source".to_string());
    VerificationOutcome::blocked(
        err.connector,
        err.source_of_truth
            .unwrap_or_else(|| "source_of_truth:unknown".to_string()),
        err.error_code,
        field_path,
        err.message,
        err.remediation,
        None,
    )
}

fn readback_payload(
    readback_id_prefix: &str,
    verifier_id: &str,
    connector: &str,
    source_of_truth: &str,
    operation: &str,
    observed_state: Value,
) -> Value {
    json!({
        "record_kind": "readback_snapshot",
        "readback_id": prefixed_id(readback_id_prefix),
        "verifier_id": verifier_id,
        "source_of_truth": {
            "kind": connector,
            "locator": source_of_truth,
        },
        "read_path": {
            "connector_id": connector,
            "operation": operation,
            "credential_key_id": null,
        },
        "observed_state": observed_state,
        "read_at": Utc::now().to_rfc3339(),
    })
}

fn latest_record_hash(conn: &Connection, path: &Path) -> BbxResult<Option<String>> {
    conn.query_row(
        "SELECT record_sha256 FROM ledger_records ORDER BY sequence DESC LIMIT 1",
        [],
        |row| row.get(0),
    )
    .optional()
    .map_err(|err| BlackboxError::sqlite(path, "read previous ledger hash", err))
}

fn latest_run_record_hash(
    conn: &Connection,
    path: &Path,
    run_id: &str,
) -> BbxResult<Option<String>> {
    conn.query_row(
        "SELECT record_sha256 FROM ledger_records
         WHERE run_id = ?1
         ORDER BY sequence DESC LIMIT 1",
        params![run_id],
        |row| row.get(0),
    )
    .optional()
    .map_err(|err| BlackboxError::sqlite(path, "read previous run hash", err))
}

#[derive(Debug)]
struct LedgerInspection {
    record_count: i64,
    ledger_tip_sha256: String,
    metadata_record_count: Option<String>,
    metadata_tip_sha256: Option<String>,
    records: Vec<LedgerRecordOutput>,
}

#[derive(Debug, Serialize)]
struct LedgerRecordOutput {
    sequence: i64,
    schema_version: i64,
    record_id: String,
    record_kind: String,
    tenant_id: String,
    run_id: Option<String>,
    step_id: Option<String>,
    created_at: String,
    actor: Value,
    payload: Value,
    hash: Value,
    redaction: Value,
}

#[derive(Debug)]
struct StoredRecord {
    sequence: i64,
    record_id: String,
    schema_version: i64,
    record_kind: String,
    tenant_id: String,
    run_id: Option<String>,
    step_id: Option<String>,
    created_at: String,
    actor_json: String,
    payload_json: String,
    redaction_json: String,
    payload_sha256: String,
    previous_record_sha256: Option<String>,
    run_previous_record_sha256: Option<String>,
    run_chain_sha256: String,
    record_sha256: String,
}

fn verify_ledger_integrity(conn: &Connection, path: &Path) -> BbxResult<LedgerInspection> {
    ensure_schema(conn, path)?;
    verify_ledger_integrity_tx_like(conn, path)
}

fn verify_ledger_integrity_tx(tx: &Transaction<'_>, path: &Path) -> BbxResult<LedgerInspection> {
    ensure_schema(tx, path)?;
    verify_ledger_integrity_tx_like(tx, path)
}

fn verify_ledger_integrity_tx_like(conn: &Connection, path: &Path) -> BbxResult<LedgerInspection> {
    let mut stmt = conn
        .prepare(
            "SELECT sequence, record_id, schema_version, record_kind, tenant_id, run_id, step_id,
                    created_at, actor_json, payload_json, redaction_json, payload_sha256,
                    previous_record_sha256, run_previous_record_sha256, run_chain_sha256,
                    record_sha256
             FROM ledger_records
             ORDER BY sequence ASC",
        )
        .map_err(|err| BlackboxError::sqlite(path, "prepare ledger integrity scan", err))?;

    let rows = stmt
        .query_map([], |row| {
            Ok(StoredRecord {
                sequence: row.get(0)?,
                record_id: row.get(1)?,
                schema_version: row.get(2)?,
                record_kind: row.get(3)?,
                tenant_id: row.get(4)?,
                run_id: row.get(5)?,
                step_id: row.get(6)?,
                created_at: row.get(7)?,
                actor_json: row.get(8)?,
                payload_json: row.get(9)?,
                redaction_json: row.get(10)?,
                payload_sha256: row.get(11)?,
                previous_record_sha256: row.get(12)?,
                run_previous_record_sha256: row.get(13)?,
                run_chain_sha256: row.get(14)?,
                record_sha256: row.get(15)?,
            })
        })
        .map_err(|err| BlackboxError::sqlite(path, "scan ledger records", err))?;

    let mut previous_record_sha256: Option<String> = None;
    let mut previous_by_run: HashMap<String, String> = HashMap::new();
    let mut outputs = Vec::new();

    for row_result in rows {
        let record =
            row_result.map_err(|err| BlackboxError::sqlite(path, "decode ledger row", err))?;
        if record.schema_version != LEDGER_SCHEMA_VERSION {
            return Err(BlackboxError::hash_chain(
                path,
                format!("ledger_records[{}].schema_version", record.sequence),
                format!(
                    "record {} has schema_version {}, expected {}",
                    record.record_id, record.schema_version, LEDGER_SCHEMA_VERSION
                ),
            ));
        }

        if record.previous_record_sha256 != previous_record_sha256 {
            return Err(BlackboxError::hash_chain(
                path,
                format!("ledger_records[{}].previous_record_sha256", record.sequence),
                format!(
                    "record {} previous hash mismatch: expected {:?}, found {:?}",
                    record.record_id, previous_record_sha256, record.previous_record_sha256
                ),
            ));
        }

        let expected_run_previous = record
            .run_id
            .as_ref()
            .and_then(|run_id| previous_by_run.get(run_id).cloned());
        if record.run_previous_record_sha256 != expected_run_previous {
            return Err(BlackboxError::hash_chain(
                path,
                format!(
                    "ledger_records[{}].run_previous_record_sha256",
                    record.sequence
                ),
                format!(
                    "record {} run previous hash mismatch: expected {:?}, found {:?}",
                    record.record_id, expected_run_previous, record.run_previous_record_sha256
                ),
            ));
        }

        let actor_value: Value = serde_json::from_str(&record.actor_json).map_err(|err| {
            BlackboxError::hash_chain(
                path,
                format!("ledger_records[{}].actor_json", record.sequence),
                format!(
                    "actor_json is not valid JSON for {}: {err}",
                    record.record_id
                ),
            )
        })?;
        let payload_value: Value = serde_json::from_str(&record.payload_json).map_err(|err| {
            BlackboxError::hash_chain(
                path,
                format!("ledger_records[{}].payload_json", record.sequence),
                format!(
                    "payload_json is not valid JSON for {}: {err}",
                    record.record_id
                ),
            )
        })?;
        let redaction_value: Value =
            serde_json::from_str(&record.redaction_json).map_err(|err| {
                BlackboxError::hash_chain(
                    path,
                    format!("ledger_records[{}].redaction_json", record.sequence),
                    format!(
                        "redaction_json is not valid JSON for {}: {err}",
                        record.record_id
                    ),
                )
            })?;

        let canonical_payload = canonical_json_string(&payload_value)?;
        let expected_payload_sha256 = sha256_hex(canonical_payload.as_bytes());
        if record.payload_sha256 != expected_payload_sha256 {
            return Err(BlackboxError::hash_chain(
                path,
                format!("ledger_records[{}].payload_sha256", record.sequence),
                format!(
                    "record {} payload hash mismatch: expected {}, found {}",
                    record.record_id, expected_payload_sha256, record.payload_sha256
                ),
            ));
        }

        let new_record = NewRecord {
            record_id: record.record_id.clone(),
            record_kind: record.record_kind.clone(),
            tenant_id: record.tenant_id.clone(),
            run_id: record.run_id.clone(),
            step_id: record.step_id.clone(),
            actor: actor_value.clone(),
            payload: payload_value.clone(),
            redaction: RedactionReport::from_json(redaction_value.clone(), path, record.sequence)?,
        };
        let actor_json = canonical_json_string(&actor_value)?;
        let redaction_json = canonical_json_string(&redaction_value)?;
        let expected_run_chain_sha256 = compute_run_chain_hash(
            &new_record,
            &record.created_at,
            &actor_json,
            &record.payload_sha256,
            &redaction_json,
            record.run_previous_record_sha256.as_deref(),
        )?;
        if record.run_chain_sha256 != expected_run_chain_sha256 {
            return Err(BlackboxError::hash_chain(
                path,
                format!("ledger_records[{}].run_chain_sha256", record.sequence),
                format!(
                    "record {} run chain mismatch: expected {}, found {}",
                    record.record_id, expected_run_chain_sha256, record.run_chain_sha256
                ),
            ));
        }

        let expected_record_sha256 = compute_record_hash(
            &new_record,
            &record.created_at,
            &actor_json,
            &record.payload_sha256,
            &redaction_json,
            record.previous_record_sha256.as_deref(),
            record.run_previous_record_sha256.as_deref(),
            &record.run_chain_sha256,
        )?;
        if record.record_sha256 != expected_record_sha256 {
            return Err(BlackboxError::hash_chain(
                path,
                format!("ledger_records[{}].record_sha256", record.sequence),
                format!(
                    "record {} hash mismatch: expected {}, found {}",
                    record.record_id, expected_record_sha256, record.record_sha256
                ),
            ));
        }

        previous_record_sha256 = Some(record.record_sha256.clone());
        if let Some(run_id) = &record.run_id {
            previous_by_run.insert(run_id.clone(), record.record_sha256.clone());
        }

        outputs.push(LedgerRecordOutput {
            sequence: record.sequence,
            schema_version: record.schema_version,
            record_id: record.record_id,
            record_kind: record.record_kind,
            tenant_id: record.tenant_id,
            run_id: record.run_id,
            step_id: record.step_id,
            created_at: record.created_at,
            actor: actor_value,
            payload: payload_value,
            hash: json!({
                "payload_sha256": record.payload_sha256,
                "previous_record_sha256": record.previous_record_sha256,
                "run_previous_record_sha256": record.run_previous_record_sha256,
                "run_chain_sha256": record.run_chain_sha256,
                "record_sha256": record.record_sha256,
            }),
            redaction: redaction_value,
        });
    }

    let metadata_count = get_metadata(conn, path, "ledger_record_count")?;
    let metadata_tip = get_metadata(conn, path, "ledger_tip_sha256")?;
    let record_count = outputs.len() as i64;
    if metadata_count.as_deref() != Some(&record_count.to_string()) {
        return Err(BlackboxError::hash_chain(
            path,
            "blackbox_metadata.ledger_record_count".to_string(),
            format!(
                "metadata record count {:?} does not match physical row count {}",
                metadata_count, record_count
            ),
        ));
    }

    let ledger_tip_sha256 = previous_record_sha256.unwrap_or_default();
    if metadata_tip.as_deref() != Some(ledger_tip_sha256.as_str()) {
        return Err(BlackboxError::hash_chain(
            path,
            "blackbox_metadata.ledger_tip_sha256".to_string(),
            format!(
                "metadata tip {:?} does not match physical ledger tip {}",
                metadata_tip, ledger_tip_sha256
            ),
        ));
    }

    Ok(LedgerInspection {
        record_count,
        ledger_tip_sha256,
        metadata_record_count: metadata_count,
        metadata_tip_sha256: metadata_tip,
        records: outputs,
    })
}

fn get_metadata(conn: &Connection, path: &Path, key: &'static str) -> BbxResult<Option<String>> {
    conn.query_row(
        "SELECT value FROM blackbox_metadata WHERE key = ?1",
        params![key],
        |row| row.get(0),
    )
    .optional()
    .map_err(|err| BlackboxError::sqlite(path, "read ledger metadata", err))
}

fn compute_run_chain_hash(
    record: &NewRecord,
    created_at: &str,
    actor_json: &str,
    payload_sha256: &str,
    redaction_json: &str,
    run_previous_record_sha256: Option<&str>,
) -> BbxResult<String> {
    let input = json!({
        "schema_version": LEDGER_SCHEMA_VERSION,
        "record_id": &record.record_id,
        "record_kind": &record.record_kind,
        "tenant_id": &record.tenant_id,
        "run_id": &record.run_id,
        "step_id": &record.step_id,
        "created_at": created_at,
        "actor_json_sha256": sha256_hex(actor_json.as_bytes()),
        "payload_sha256": payload_sha256,
        "redaction_json_sha256": sha256_hex(redaction_json.as_bytes()),
        "run_previous_record_sha256": run_previous_record_sha256,
    });
    Ok(sha256_hex(&canonical_json_bytes(&input)?))
}

fn compute_record_hash(
    record: &NewRecord,
    created_at: &str,
    actor_json: &str,
    payload_sha256: &str,
    redaction_json: &str,
    previous_record_sha256: Option<&str>,
    run_previous_record_sha256: Option<&str>,
    run_chain_sha256: &str,
) -> BbxResult<String> {
    let input = json!({
        "schema_version": LEDGER_SCHEMA_VERSION,
        "record_id": &record.record_id,
        "record_kind": &record.record_kind,
        "tenant_id": &record.tenant_id,
        "run_id": &record.run_id,
        "step_id": &record.step_id,
        "created_at": created_at,
        "actor_json_sha256": sha256_hex(actor_json.as_bytes()),
        "payload_sha256": payload_sha256,
        "redaction_json_sha256": sha256_hex(redaction_json.as_bytes()),
        "previous_record_sha256": previous_record_sha256,
        "run_previous_record_sha256": run_previous_record_sha256,
        "run_chain_sha256": run_chain_sha256,
    });
    Ok(sha256_hex(&canonical_json_bytes(&input)?))
}

#[derive(Debug, Clone)]
struct RedactionReport {
    redacted_fields: Vec<String>,
    secret_digest_fields: BTreeMap<String, String>,
}

impl RedactionReport {
    fn empty() -> Self {
        Self {
            redacted_fields: Vec::new(),
            secret_digest_fields: BTreeMap::new(),
        }
    }

    fn to_json(&self) -> Value {
        json!({
            "policy_id": REDACTION_POLICY_ID,
            "redacted_fields": self.redacted_fields,
            "secret_digest_fields": self.secret_digest_fields,
        })
    }

    fn from_json(value: Value, ledger_path: &Path, sequence: i64) -> BbxResult<Self> {
        let policy = value
            .get("policy_id")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                BlackboxError::hash_chain(
                    ledger_path,
                    format!("ledger_records[{sequence}].redaction_json.policy_id"),
                    "redaction policy_id missing",
                )
            })?;
        if policy != REDACTION_POLICY_ID {
            return Err(BlackboxError::hash_chain(
                ledger_path,
                format!("ledger_records[{sequence}].redaction_json.policy_id"),
                format!("unsupported redaction policy_id {policy}"),
            ));
        }

        let redacted_fields = value
            .get("redacted_fields")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                BlackboxError::hash_chain(
                    ledger_path,
                    format!("ledger_records[{sequence}].redaction_json.redacted_fields"),
                    "redacted_fields missing",
                )
            })?
            .iter()
            .map(|item| {
                item.as_str().map(ToOwned::to_owned).ok_or_else(|| {
                    BlackboxError::hash_chain(
                        ledger_path,
                        format!("ledger_records[{sequence}].redaction_json.redacted_fields"),
                        "redacted_fields must contain strings",
                    )
                })
            })
            .collect::<BbxResult<Vec<_>>>()?;

        let secret_digest_fields = value
            .get("secret_digest_fields")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                BlackboxError::hash_chain(
                    ledger_path,
                    format!("ledger_records[{sequence}].redaction_json.secret_digest_fields"),
                    "secret_digest_fields missing",
                )
            })?
            .iter()
            .map(|(key, val)| {
                val.as_str()
                    .map(|digest| (key.clone(), digest.to_string()))
                    .ok_or_else(|| {
                        BlackboxError::hash_chain(
                            ledger_path,
                            format!(
                                "ledger_records[{sequence}].redaction_json.secret_digest_fields.{key}"
                            ),
                            "secret_digest_fields values must be strings",
                        )
                    })
            })
            .collect::<BbxResult<BTreeMap<_, _>>>()?;

        Ok(Self {
            redacted_fields,
            secret_digest_fields,
        })
    }
}

#[derive(Debug)]
struct RedactedInput {
    value: Value,
    report: RedactionReport,
}

fn redact_tool_input(path: &Path, input: &Value) -> BbxResult<RedactedInput> {
    let mut report = RedactionReport::empty();
    let value = redact_value(path, input, "$", &mut report)?;
    scan_for_unredacted_secret(path, "$", &value)?;
    Ok(RedactedInput { value, report })
}

fn redact_value(
    ledger_path: &Path,
    value: &Value,
    path: &str,
    report: &mut RedactionReport,
) -> BbxResult<Value> {
    match value {
        Value::Object(map) => {
            let mut redacted = Map::new();
            for (key, child) in map {
                let child_path = format!("{path}.{key}");
                if is_sensitive_field_name(key) {
                    let digest = sha256_hex(canonical_json_string(child)?.as_bytes());
                    report.redacted_fields.push(child_path.clone());
                    report
                        .secret_digest_fields
                        .insert(child_path, digest.clone());
                    redacted.insert(key.clone(), Value::String(format!("[REDACTED:{digest}]")));
                } else {
                    redacted.insert(
                        key.clone(),
                        redact_value(ledger_path, child, &child_path, report)?,
                    );
                }
            }
            Ok(Value::Object(redacted))
        }
        Value::Array(items) => {
            let mut redacted = Vec::with_capacity(items.len());
            for (idx, item) in items.iter().enumerate() {
                redacted.push(redact_value(
                    ledger_path,
                    item,
                    &format!("{path}[{idx}]"),
                    report,
                )?);
            }
            Ok(Value::Array(redacted))
        }
        Value::String(text) if looks_like_secret_literal(text) => Err(BlackboxError::redaction(
            ledger_path,
            path.to_string(),
            "secret-like value found outside an explicitly redacted field",
        )),
        other => Ok(other.clone()),
    }
}

fn scan_for_unredacted_secret(ledger_path: &Path, path: &str, value: &Value) -> BbxResult<()> {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                scan_for_unredacted_secret(ledger_path, &format!("{path}.{key}"), child)?;
            }
        }
        Value::Array(items) => {
            for (idx, child) in items.iter().enumerate() {
                scan_for_unredacted_secret(ledger_path, &format!("{path}[{idx}]"), child)?;
            }
        }
        Value::String(text)
            if looks_like_secret_literal(text) && !text.starts_with("[REDACTED:") =>
        {
            return Err(BlackboxError::redaction(
                ledger_path,
                path.to_string(),
                "redaction post-scan found an unredacted secret-like value",
            ));
        }
        _ => {}
    }
    Ok(())
}

fn is_sensitive_field_name(key: &str) -> bool {
    let normalized = key
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect::<String>();
    matches!(
        normalized.as_str(),
        "password"
            | "passwd"
            | "pwd"
            | "token"
            | "apikey"
            | "accesstoken"
            | "refreshtoken"
            | "authorization"
            | "secret"
            | "clientsecret"
            | "privatekey"
            | "sshkey"
            | "connectionstring"
    ) || normalized.ends_with("token")
        || normalized.ends_with("secret")
        || normalized.ends_with("apikey")
}

fn looks_like_secret_literal(text: &str) -> bool {
    let trimmed = text.trim();
    let lower = trimmed.to_ascii_lowercase();
    lower.contains("bearer ")
        || lower.contains("-----begin ")
        || lower.contains("private key-----")
        || lower.contains("api_key=")
        || lower.contains("apikey=")
        || lower.contains("access_token=")
        || lower.contains("password=")
        || lower.contains("client_secret=")
        || lower.starts_with("ghp_")
        || lower.starts_with("github_pat_")
        || lower.starts_with("xoxb-")
        || lower.starts_with("sk-")
        || looks_like_aws_access_key(trimmed)
        || looks_like_jwt(trimmed)
}

fn looks_like_aws_access_key(text: &str) -> bool {
    text.len() == 20
        && text.to_ascii_uppercase().starts_with("AKIA")
        && text.chars().all(|c| c.is_ascii_alphanumeric())
}

fn looks_like_jwt(text: &str) -> bool {
    let parts = text.split('.').collect::<Vec<_>>();
    parts.len() == 3
        && parts.iter().all(|part| {
            part.len() >= 8
                && part
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        })
        && parts[0].starts_with("eyJ")
}

fn actor_json(agent_id: &str, model_id: Option<&str>) -> Value {
    json!({
        "actor_kind": "ai_agent",
        "agent_id": agent_id,
        "model_id": model_id,
        "human_approver_id": null,
    })
}

fn prefixed_id(prefix: &str) -> String {
    format!("{prefix}_{}", Uuid::new_v4().simple())
}

fn source_of_truth(path: &Path) -> String {
    format!("sqlite:{}#ledger_records", path.display())
}

fn canonical_json_string(value: &Value) -> BbxResult<String> {
    let bytes = canonical_json_bytes(value)?;
    String::from_utf8(bytes).map_err(|err| {
        BlackboxError::new(
            ErrorKind::InvalidInput,
            "BBX_JSON_CANONICALIZATION_FAILED",
            format!("canonical JSON was not UTF-8: {err}"),
            None,
            None,
            "report this as a bug; JSON serialization should always emit UTF-8",
        )
    })
}

fn canonical_json_bytes(value: &Value) -> BbxResult<Vec<u8>> {
    let canonical = canonicalize(value);
    serde_json::to_vec(&canonical).map_err(|err| {
        BlackboxError::new(
            ErrorKind::InvalidInput,
            "BBX_JSON_CANONICALIZATION_FAILED",
            format!("failed to serialize canonical JSON: {err}"),
            None,
            None,
            "fix the JSON value and retry",
        )
    })
}

fn canonicalize(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut sorted = BTreeMap::new();
            for (key, val) in map {
                sorted.insert(key.clone(), canonicalize(val));
            }
            let mut out = Map::new();
            for (key, val) in sorted {
                out.insert(key, val);
            }
            Value::Object(out)
        }
        Value::Array(items) => Value::Array(items.iter().map(canonicalize).collect()),
        other => other.clone(),
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity("sha256:".len() + digest.len() * 2);
    out.push_str("sha256:");
    for byte in digest {
        out.push(nibble_to_hex(byte >> 4));
        out.push(nibble_to_hex(byte & 0x0f));
    }
    out
}

fn nibble_to_hex(nibble: u8) -> char {
    match nibble {
        0..=9 => (b'0' + nibble) as char,
        10..=15 => (b'a' + (nibble - 10)) as char,
        _ => unreachable!("nibble is always four bits"),
    }
}
