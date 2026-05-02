//! BlackBoxFix Phase 1 local ledger and CLI.
//!
//! Source of truth: SQLite table `ledger_records` in the user-provided
//! ledger database. Every mutating command writes only after redaction,
//! schema validation, and hash-chain verification have succeeded.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
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

const LEDGER_SCHEMA_VERSION: i64 = 4;
const REDACTION_POLICY_ID: &str = "redact_v1";
const SQLITE_CONNECTOR_ID: &str = "sqlite_local_v1";
const GIT_CONNECTOR_ID: &str = "git_local_v1";
const FS_CONNECTOR_ID: &str = "fs_local_v1";
const CMD_CONNECTOR_ID: &str = "cmd_local_v1";
const GITHUB_CONNECTOR_ID: &str = "github_v1";
const GHA_CONNECTOR_ID: &str = "gha_v1";
const MCP_CONNECTOR_ID: &str = "mcp_stdio_v1";
const POLICY_CONNECTOR_ID: &str = "blackbox_policy_v1";
const GITHUB_API_BASE: &str = "https://api.github.com";
const GITHUB_API_VERSION: &str = "2022-11-28";
const GITHUB_USER_AGENT: &str = "contextgraph-blackbox/0.1";
const EPISODE_EXPORT_SCHEMA_VERSION: i64 = 1;
const DASHBOARD_SCHEMA_VERSION: i64 = 1;
const MCP_PROTOCOL_VERSION: &str = "2025-06-18";
const VERIFIED_GIT_APPLY_PATCH_TOOL: &str = "verified_git_apply_patch";
const VERIFIED_GIT_APPLY_PATCH_CAPABILITY: &str = "local.git.apply_patch";
const VERIFIED_GIT_APPLY_PATCH_VERIFIER: &str = "git_file_diff_matches_patch_v1";

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
    /// Evaluate and persist Phase 6 policy decisions.
    Policy {
        #[command(subcommand)]
        action: PolicyCommands,
    },
    /// Run a Phase 6 MCP JSON-RPC stdio server.
    Mcp {
        #[command(subcommand)]
        action: McpCommands,
    },
    /// Inspect Phase 6 API surface metadata.
    Api {
        #[command(subcommand)]
        action: ApiCommands,
    },
    /// Verify a tool call against local source-of-truth systems.
    Verify(VerifyArgs),
    /// Reconstruct a run timeline from immutable ledger records without mutation.
    Replay(ReplayArgs),
    /// Export BlackBoxFix evidence reports.
    Export {
        #[command(subcommand)]
        action: ExportCommands,
    },
    /// Build and inspect Phase 7 dashboard API responses.
    Dashboard {
        #[command(subcommand)]
        action: DashboardCommands,
    },
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
    /// Record one agent reasoning/action step inside an existing run.
    Step(RecordAgentStepArgs),
    /// Record a model tool-choice decision inside an existing run.
    #[command(name = "model-decision")]
    ModelDecision(RecordModelDecisionArgs),
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
pub struct RecordAgentStepArgs {
    /// SQLite ledger database path.
    #[arg(long, env = "BLACKBOX_LEDGER")]
    pub ledger: PathBuf,

    /// Existing run id.
    #[arg(long)]
    pub run_id: String,

    /// Step kind, for example model_decision or tool_execution.
    #[arg(long, default_value = "agent_step")]
    pub step_kind: String,

    /// Human-readable step summary.
    #[arg(long)]
    pub summary: String,

    /// Tenant identifier for local mode.
    #[arg(long, default_value = "tenant_local")]
    pub tenant: String,

    /// Agent identifier.
    #[arg(long, default_value = "agent_unknown")]
    pub agent: String,

    /// Optional model identifier.
    #[arg(long)]
    pub model: Option<String>,
}

#[derive(Args, Debug)]
pub struct RecordModelDecisionArgs {
    /// SQLite ledger database path.
    #[arg(long, env = "BLACKBOX_LEDGER")]
    pub ledger: PathBuf,

    /// Existing run id.
    #[arg(long)]
    pub run_id: String,

    /// Tool selected by the model or adapter.
    #[arg(long)]
    pub tool_name: String,

    /// Capability selected by the model or adapter.
    #[arg(long)]
    pub capability: String,

    /// Confidence in [0,1].
    #[arg(long, default_value_t = 1.0)]
    pub confidence: f64,

    /// JSON file containing redacted model/input context.
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

#[derive(Subcommand, Debug)]
pub enum PolicyCommands {
    /// Evaluate one tool action and persist the policy decision row.
    Evaluate(PolicyEvaluateArgs),
    /// List the shipped Phase 6 policy modes and capability registry.
    Registry(PolicyRegistryArgs),
}

#[derive(Args, Debug)]
pub struct PolicyEvaluateArgs {
    /// SQLite ledger database path.
    #[arg(long, env = "BLACKBOX_LEDGER")]
    pub ledger: PathBuf,

    /// Policy mode to apply.
    #[arg(long, value_enum)]
    pub mode: PolicyMode,

    /// Tool name being authorized.
    #[arg(long)]
    pub tool: String,

    /// Capability identifier being authorized.
    #[arg(long)]
    pub capability: String,

    /// Existing run id, when the action belongs to a run.
    #[arg(long)]
    pub run_id: Option<String>,

    /// Whether the action can mutate an external source of truth.
    #[arg(long)]
    pub mutating: bool,

    /// Verifier id that will independently read source-of-truth state.
    #[arg(long)]
    pub verifier_id: Option<String>,

    /// Source-of-truth scope string for audit and policy matching.
    #[arg(long)]
    pub source_scope: Option<String>,

    /// Human approval id required by supervised mode.
    #[arg(long)]
    pub approval_id: Option<String>,

    /// Human approver id required by supervised mode.
    #[arg(long)]
    pub approved_by: Option<String>,

    /// Tenant identifier for local mode.
    #[arg(long, default_value = "tenant_local")]
    pub tenant: String,

    /// Agent identifier.
    #[arg(long, default_value = "agent_unknown")]
    pub agent: String,

    /// Optional model identifier.
    #[arg(long)]
    pub model: Option<String>,
}

#[derive(Args, Debug)]
pub struct PolicyRegistryArgs {
    /// SQLite ledger database path. When supplied, registry output also verifies the ledger.
    #[arg(long, env = "BLACKBOX_LEDGER")]
    pub ledger: Option<PathBuf>,
}

#[derive(Subcommand, Debug)]
pub enum McpCommands {
    /// Recorder MCP server: ledger recording and verifier tools over JSON-RPC stdio.
    Recorder(McpServerArgs),
    /// Gateway MCP server: verified mutating wrappers with policy enforcement.
    Gateway(McpGatewayArgs),
}

#[derive(Args, Debug)]
pub struct McpServerArgs {
    /// SQLite ledger database path.
    #[arg(long, env = "BLACKBOX_LEDGER")]
    pub ledger: PathBuf,
}

#[derive(Args, Debug)]
pub struct McpGatewayArgs {
    /// SQLite ledger database path.
    #[arg(long, env = "BLACKBOX_LEDGER")]
    pub ledger: PathBuf,

    /// Policy mode enforced by gateway tools.
    #[arg(long, default_value = "bounded_autonomous", value_enum)]
    pub policy_mode: PolicyMode,
}

#[derive(Subcommand, Debug)]
pub enum ApiCommands {
    /// Print the Phase 6 HTTP API endpoint and schema surface.
    Surface(ApiSurfaceArgs),
}

#[derive(Args, Debug)]
pub struct ApiSurfaceArgs {
    /// SQLite ledger database path. When supplied, output verifies the ledger first.
    #[arg(long, env = "BLACKBOX_LEDGER")]
    pub ledger: Option<PathBuf>,
}

#[derive(Args, Debug)]
pub struct ReplayArgs {
    /// SQLite ledger database path.
    #[arg(long, env = "BLACKBOX_LEDGER")]
    pub ledger: PathBuf,

    /// Existing run id to reconstruct.
    #[arg(long)]
    pub run_id: String,
}

#[derive(Subcommand, Debug)]
pub enum ExportCommands {
    /// Export a JSON evidence report for a run.
    Evidence(ExportEvidenceArgs),
    /// Export verified training/eval episodes from the ledger.
    Episodes(ExportEpisodesArgs),
}

#[derive(Args, Debug)]
pub struct ExportEvidenceArgs {
    /// SQLite ledger database path.
    #[arg(long, env = "BLACKBOX_LEDGER")]
    pub ledger: PathBuf,

    /// Existing run id to export.
    #[arg(long)]
    pub run_id: String,

    /// Output JSON file path.
    #[arg(long)]
    pub out: PathBuf,
}

#[derive(Args, Debug)]
pub struct ExportEpisodesArgs {
    /// SQLite ledger database path.
    #[arg(long, env = "BLACKBOX_LEDGER")]
    pub ledger: PathBuf,

    /// Output directory. The exporter writes episodes.jsonl, manifest.json, and evals/.
    #[arg(long)]
    pub out: PathBuf,

    /// Replace an existing output directory.
    #[arg(long)]
    pub force: bool,
}

#[derive(Subcommand, Debug)]
pub enum DashboardCommands {
    /// Build a static dashboard bundle from the verified ledger.
    Build(DashboardBuildArgs),
    /// Emit one dashboard API response from the verified ledger.
    Api(DashboardApiArgs),
}

#[derive(Args, Debug)]
pub struct DashboardBuildArgs {
    /// SQLite ledger database path.
    #[arg(long, env = "BLACKBOX_LEDGER")]
    pub ledger: PathBuf,

    /// Output directory. The builder writes index.html, manifest.json, and api/*.json.
    #[arg(long)]
    pub out: PathBuf,

    /// Replace an existing dashboard directory.
    #[arg(long)]
    pub force: bool,
}

#[derive(Args, Debug)]
pub struct DashboardApiArgs {
    /// SQLite ledger database path.
    #[arg(long, env = "BLACKBOX_LEDGER")]
    pub ledger: PathBuf,

    /// Dashboard view to emit.
    #[arg(long, value_enum)]
    pub view: DashboardView,

    /// Required for --view timeline.
    #[arg(long)]
    pub run_id: Option<String>,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
pub enum DashboardView {
    Summary,
    Runs,
    Timeline,
    Evidence,
    Verifiers,
    Errors,
    Exports,
}

impl DashboardView {
    fn as_str(self) -> &'static str {
        match self {
            Self::Summary => "summary",
            Self::Runs => "runs",
            Self::Timeline => "timeline",
            Self::Evidence => "evidence",
            Self::Verifiers => "verifiers",
            Self::Errors => "errors",
            Self::Exports => "exports",
        }
    }
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

#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
pub enum PolicyMode {
    #[value(name = "observe_only")]
    ObserveOnly,
    #[value(name = "supervised")]
    Supervised,
    #[value(name = "bounded_autonomous")]
    BoundedAutonomous,
    #[value(name = "blocked")]
    Blocked,
}

impl PolicyMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::ObserveOnly => "observe_only",
            Self::Supervised => "supervised",
            Self::BoundedAutonomous => "bounded_autonomous",
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

    fn evidence_integrity(
        error_code: &'static str,
        source_of_truth: String,
        field_path: impl Into<String>,
        message: impl Into<String>,
        remediation: impl Into<String>,
    ) -> Self {
        Self::evidence_integrity_for_connector(
            error_code,
            SQLITE_CONNECTOR_ID,
            source_of_truth,
            field_path,
            message,
            remediation,
        )
    }

    fn evidence_integrity_for_connector(
        error_code: &'static str,
        connector: &'static str,
        source_of_truth: String,
        field_path: impl Into<String>,
        message: impl Into<String>,
        remediation: impl Into<String>,
    ) -> Self {
        Self::new_with_connector(
            ErrorKind::HashChain,
            error_code,
            message,
            Some(source_of_truth),
            connector,
            Some(field_path.into()),
            remediation,
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
    match command {
        BlackboxCommands::Mcp { action } => return handle_mcp_command(action).await,
        other => return handle_blackbox_json_command(other).await,
    }
}

async fn handle_blackbox_json_command(command: BlackboxCommands) -> i32 {
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
            RecordCommands::Step(args) => record_agent_step(args),
            RecordCommands::ModelDecision(args) => record_model_decision(args),
            RecordCommands::ToolCall(args) => record_tool_call(args),
        },
        BlackboxCommands::Policy { action } => match action {
            PolicyCommands::Evaluate(args) => policy_evaluate(args),
            PolicyCommands::Registry(args) => policy_registry(args),
        },
        BlackboxCommands::Mcp { action: _ } => Err(BlackboxError::invalid(
            "MCP server commands must be handled through the stdio server path",
            "report this as a bug; handle_blackbox_command should intercept MCP commands",
        )),
        BlackboxCommands::Api { action } => match action {
            ApiCommands::Surface(args) => api_surface(args),
        },
        BlackboxCommands::Verify(args) => verify_local(args).await,
        BlackboxCommands::Replay(args) => replay_run(args),
        BlackboxCommands::Export { action } => match action {
            ExportCommands::Evidence(args) => export_evidence(args),
            ExportCommands::Episodes(args) => export_episodes(args),
        },
        BlackboxCommands::Dashboard { action } => match action {
            DashboardCommands::Build(args) => dashboard_build(args),
            DashboardCommands::Api(args) => dashboard_api(args),
        },
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

fn record_agent_step(args: RecordAgentStepArgs) -> BbxResult<Value> {
    record_agent_step_from_parts(
        &args.ledger,
        &args.run_id,
        &args.step_kind,
        &args.summary,
        &args.tenant,
        &args.agent,
        args.model.as_deref(),
        json!({}),
    )
}

fn record_agent_step_from_parts(
    ledger: &Path,
    run_id: &str,
    step_kind: &str,
    summary: &str,
    tenant: &str,
    agent: &str,
    model: Option<&str>,
    context: Value,
) -> BbxResult<Value> {
    validate_ledger_path(ledger)?;
    require_non_empty_for_ledger(ledger, "--run-id", run_id)?;
    require_non_empty_for_ledger(ledger, "--step-kind", step_kind)?;
    require_non_empty_for_ledger(ledger, "--summary", summary)?;
    require_non_empty_for_ledger(ledger, "--tenant", tenant)?;
    require_non_empty_for_ledger(ledger, "--agent", agent)?;

    let mut conn = open_initialized_ledger(ledger)?;
    ensure_run_exists_for_tenant(&conn, ledger, run_id, tenant)?;
    let step_id = prefixed_id("step");
    let record_id = prefixed_id("bbx_rec");
    let actor = actor_json(agent, model);
    let payload = json!({
        "record_kind": "agent_step",
        "step_id": step_id,
        "run_id": run_id,
        "step_kind": step_kind,
        "summary": summary,
        "context": context,
        "recorded_at": Utc::now().to_rfc3339(),
    });
    let append = append_record(
        &mut conn,
        ledger,
        NewRecord {
            record_id: record_id.clone(),
            record_kind: "agent_step".to_string(),
            tenant_id: tenant.to_string(),
            run_id: Some(run_id.to_string()),
            step_id: Some(step_id.clone()),
            actor,
            payload,
            redaction: RedactionReport::empty(),
        },
    )?;
    Ok(json!({
        "status": "ok",
        "command": "blackbox record step",
        "record_id": record_id,
        "record_kind": "agent_step",
        "run_id": run_id,
        "step_id": step_id,
        "ledger_sequence": append.sequence,
        "source_of_truth": source_of_truth(ledger),
        "payload_sha256": append.payload_sha256,
        "record_sha256": append.record_sha256,
    }))
}

fn record_model_decision(args: RecordModelDecisionArgs) -> BbxResult<Value> {
    validate_ledger_path(&args.ledger)?;
    let input_text = fs::read_to_string(&args.input)
        .map_err(|err| BlackboxError::io(&args.input, "read model decision input JSON", err))?;
    let input_value: Value = serde_json::from_str(&input_text).map_err(|err| {
        BlackboxError::new(
            ErrorKind::InvalidInput,
            "BBX_INPUT_JSON_INVALID",
            format!("model decision input is not valid JSON: {err}"),
            Some(format!("file:{}", args.input.display())),
            Some("$".to_string()),
            "write valid JSON to the input file and retry",
        )
    })?;
    record_model_decision_from_value(
        &args.ledger,
        &args.run_id,
        &args.tool_name,
        &args.capability,
        args.confidence,
        &input_value,
        &args.tenant,
        &args.agent,
        args.model.as_deref(),
        None,
    )
}

#[allow(clippy::too_many_arguments)]
fn record_model_decision_from_value(
    ledger: &Path,
    run_id: &str,
    tool_name: &str,
    capability_id: &str,
    confidence: f64,
    input_value: &Value,
    tenant: &str,
    agent: &str,
    model: Option<&str>,
    linked_step_id: Option<&str>,
) -> BbxResult<Value> {
    validate_ledger_path(ledger)?;
    require_non_empty_for_ledger(ledger, "--run-id", run_id)?;
    require_non_empty_for_ledger(ledger, "--tool-name", tool_name)?;
    require_non_empty_for_ledger(ledger, "--capability", capability_id)?;
    require_non_empty_for_ledger(ledger, "--tenant", tenant)?;
    require_non_empty_for_ledger(ledger, "--agent", agent)?;
    if !(0.0..=1.0).contains(&confidence) {
        return Err(BlackboxError::invalid_field_for_ledger(
            ledger,
            "--confidence",
            format!("confidence must be in [0,1], got {confidence}"),
            "pass a numeric confidence between 0.0 and 1.0",
        ));
    }
    if !input_value.is_object() {
        return Err(BlackboxError::new(
            ErrorKind::InvalidInput,
            "BBX_INPUT_SCHEMA_INVALID",
            "model decision input JSON must be an object",
            Some(source_of_truth(ledger)),
            Some("$.input".to_string()),
            "wrap the model decision context in a JSON object",
        ));
    }

    let mut conn = open_initialized_ledger(ledger)?;
    ensure_run_exists_for_tenant(&conn, ledger, run_id, tenant)?;
    let redacted = redact_tool_input(ledger, input_value)?;
    let model_decision_id = prefixed_id("decision");
    let step_id = linked_step_id
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| prefixed_id("step"));
    let record_id = prefixed_id("bbx_rec");
    let actor = actor_json(agent, model);
    let payload = json!({
        "record_kind": "model_decision",
        "model_decision_id": model_decision_id,
        "run_id": run_id,
        "step_id": step_id,
        "tool_choice": {
            "tool_name": tool_name,
            "capability_id": capability_id,
        },
        "confidence": confidence,
        "policy_context": redacted.value.get("policy_context").cloned().unwrap_or(Value::Null),
        "input_redacted": redacted.value,
        "recorded_at": Utc::now().to_rfc3339(),
    });
    let append = append_record(
        &mut conn,
        ledger,
        NewRecord {
            record_id: record_id.clone(),
            record_kind: "model_decision".to_string(),
            tenant_id: tenant.to_string(),
            run_id: Some(run_id.to_string()),
            step_id: Some(step_id.clone()),
            actor,
            payload,
            redaction: redacted.report,
        },
    )?;
    Ok(json!({
        "status": "ok",
        "command": "blackbox record model-decision",
        "record_id": record_id,
        "record_kind": "model_decision",
        "run_id": run_id,
        "step_id": step_id,
        "model_decision_id": model_decision_id,
        "ledger_sequence": append.sequence,
        "source_of_truth": source_of_truth(ledger),
        "payload_sha256": append.payload_sha256,
        "record_sha256": append.record_sha256,
        "redaction": append.redaction,
    }))
}

fn record_tool_call(args: RecordToolCallArgs) -> BbxResult<Value> {
    validate_ledger_path(&args.ledger)?;
    require_non_empty_for_ledger(&args.ledger, "--run-id", &args.run_id)?;
    require_non_empty_for_ledger(&args.ledger, "--tool", &args.tool)?;
    require_non_empty_for_ledger(&args.ledger, "--tenant", &args.tenant)?;
    require_non_empty_for_ledger(&args.ledger, "--agent", &args.agent)?;
    require_non_empty_for_ledger(&args.ledger, "--capability", &args.capability)?;

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

    record_tool_call_from_value(
        &args.ledger,
        &args.run_id,
        &args.tool,
        &input_value,
        &args.tenant,
        &args.agent,
        args.model.as_deref(),
        &args.capability,
        args.status,
        args.external_request_id.as_deref(),
        json!({
            "policy_decision_id": null,
            "mode": "local",
            "approved_by": null,
        }),
        None,
    )
}

#[allow(clippy::too_many_arguments)]
fn record_tool_call_from_value(
    ledger: &Path,
    run_id: &str,
    tool_name: &str,
    input_value: &Value,
    tenant: &str,
    agent: &str,
    model: Option<&str>,
    capability: &str,
    status: ToolExecutionStatus,
    external_request_id: Option<&str>,
    authorization: Value,
    step_id: Option<&str>,
) -> BbxResult<Value> {
    validate_ledger_path(ledger)?;
    require_non_empty_for_ledger(ledger, "--run-id", run_id)?;
    require_non_empty_for_ledger(ledger, "--tool", tool_name)?;
    require_non_empty_for_ledger(ledger, "--tenant", tenant)?;
    require_non_empty_for_ledger(ledger, "--agent", agent)?;
    require_non_empty_for_ledger(ledger, "--capability", capability)?;
    if !input_value.is_object() {
        return Err(BlackboxError::new(
            ErrorKind::InvalidInput,
            "BBX_INPUT_SCHEMA_INVALID",
            "tool input JSON must be an object",
            Some(source_of_truth(ledger)),
            Some("$.input".to_string()),
            "wrap the tool input in a JSON object so field-level redaction can be verified",
        ));
    }

    let mut conn = open_initialized_ledger(ledger)?;
    ensure_run_exists_for_tenant(&conn, ledger, run_id, tenant)?;
    let redacted = redact_tool_input(ledger, input_value)?;
    let canonical_redacted = canonical_json_bytes(&redacted.value)?;
    let input_digest = sha256_hex(&canonical_redacted);

    let tool_call_id = prefixed_id("tool");
    let record_id = prefixed_id("bbx_rec");
    let now = Utc::now().to_rfc3339();
    let actor = actor_json(agent, model);
    let payload = json!({
        "record_kind": "tool_call",
        "tool_call_id": tool_call_id,
        "run_id": run_id,
        "tool_name": tool_name,
        "capability_id": capability,
        "input_digest": input_digest,
        "input_redacted": redacted.value,
        "authorization": authorization,
        "execution": {
            "started_at": now,
            "finished_at": now,
            "status": status.as_str(),
            "external_request_id": external_request_id,
        },
    });

    let append = append_record(
        &mut conn,
        ledger,
        NewRecord {
            record_id: record_id.clone(),
            record_kind: "tool_call".to_string(),
            tenant_id: tenant.to_string(),
            run_id: Some(run_id.to_string()),
            step_id: step_id.map(ToOwned::to_owned),
            actor,
            payload,
            redaction: redacted.report,
        },
    )?;

    Ok(json!({
        "status": "ok",
        "record_id": record_id,
        "record_kind": "tool_call",
        "run_id": run_id,
        "tool_call_id": tool_call_id,
        "ledger_sequence": append.sequence,
        "source_of_truth": source_of_truth(ledger),
        "payload_sha256": append.payload_sha256,
        "previous_record_sha256": append.previous_record_sha256,
        "run_chain_sha256": append.run_chain_sha256,
        "record_sha256": append.record_sha256,
        "redaction": append.redaction,
    }))
}

fn policy_registry(args: PolicyRegistryArgs) -> BbxResult<Value> {
    let ledger_source = if let Some(ledger) = args.ledger {
        let conn = open_initialized_ledger(&ledger)?;
        verify_ledger_integrity(&conn, &ledger)?;
        Value::String(source_of_truth(&ledger))
    } else {
        Value::Null
    };
    Ok(json!({
        "status": "ok",
        "command": "blackbox policy registry",
        "source_of_truth": ledger_source,
        "policy_modes": policy_modes_json(),
        "capabilities": capability_registry_json(),
        "mcp_tools": mcp_tool_names(true),
        "fail_closed": true,
    }))
}

fn policy_evaluate(args: PolicyEvaluateArgs) -> BbxResult<Value> {
    policy_evaluate_from_parts(PolicyEvaluateInput {
        ledger: &args.ledger,
        mode: args.mode,
        run_id: args.run_id.as_deref(),
        tool_name: &args.tool,
        capability_id: &args.capability,
        mutating: args.mutating,
        verifier_id: args.verifier_id.as_deref(),
        source_scope: args.source_scope.as_deref(),
        approval_id: args.approval_id.as_deref(),
        approved_by: args.approved_by.as_deref(),
        tenant: &args.tenant,
        agent: &args.agent,
        model: args.model.as_deref(),
    })
}

struct PolicyEvaluateInput<'a> {
    ledger: &'a Path,
    mode: PolicyMode,
    run_id: Option<&'a str>,
    tool_name: &'a str,
    capability_id: &'a str,
    mutating: bool,
    verifier_id: Option<&'a str>,
    source_scope: Option<&'a str>,
    approval_id: Option<&'a str>,
    approved_by: Option<&'a str>,
    tenant: &'a str,
    agent: &'a str,
    model: Option<&'a str>,
}

fn policy_evaluate_from_parts(input: PolicyEvaluateInput<'_>) -> BbxResult<Value> {
    validate_ledger_path(input.ledger)?;
    require_non_empty_for_ledger(input.ledger, "--tool", input.tool_name)?;
    require_non_empty_for_ledger(input.ledger, "--capability", input.capability_id)?;
    require_non_empty_for_ledger(input.ledger, "--tenant", input.tenant)?;
    require_non_empty_for_ledger(input.ledger, "--agent", input.agent)?;
    if let Some(run_id) = input.run_id {
        require_non_empty_for_ledger(input.ledger, "--run-id", run_id)?;
    }

    let mut conn = open_initialized_ledger(input.ledger)?;
    if let Some(run_id) = input.run_id {
        ensure_run_exists_for_tenant(&conn, input.ledger, run_id, input.tenant)?;
    }

    let capability = capability_by_id(input.capability_id);
    let mut status = "allowed";
    let mut error_code: Option<&'static str> = None;
    let mut field_path: Option<String> = None;
    let mut remediation: Option<&'static str> = None;
    if input.mode == PolicyMode::Blocked {
        status = "denied";
        error_code = Some("POLICY_MODE_BLOCKED");
        field_path = Some("policy.mode".to_string());
        remediation = Some("choose a non-blocked policy mode after an explicit governance change");
    } else if capability.is_none() {
        status = "denied";
        error_code = Some("POLICY_CAPABILITY_UNKNOWN");
        field_path = Some("capability_id".to_string());
        remediation =
            Some("register the capability with verifier coverage before allowing the action");
    } else if input.mutating
        && input
            .verifier_id
            .filter(|id| !id.trim().is_empty())
            .is_none()
    {
        status = "denied";
        error_code = Some("POLICY_VERIFIER_REQUIRED");
        field_path = Some("verifier_id".to_string());
        remediation =
            Some("attach an independent source-of-truth verifier to every mutating action");
    } else if let Some(capability) = capability {
        if input.mutating != capability.mutating {
            status = "denied";
            error_code = Some("POLICY_CAPABILITY_MUTATION_MISMATCH");
            field_path = Some("mutating".to_string());
            remediation = Some("use the capability definition's mutating flag exactly");
        } else if input.mutating && input.verifier_id != Some(capability.verifier_id) {
            status = "denied";
            error_code = Some("POLICY_VERIFIER_MISMATCH");
            field_path = Some("verifier_id".to_string());
            remediation = Some("use the verifier registered for the requested mutating capability");
        } else if input.mode == PolicyMode::Supervised
            && (input
                .approval_id
                .filter(|value| !value.trim().is_empty())
                .is_none()
                || input
                    .approved_by
                    .filter(|value| !value.trim().is_empty())
                    .is_none())
        {
            status = "denied";
            error_code = Some("POLICY_APPROVAL_REQUIRED");
            field_path = Some("approval".to_string());
            remediation = Some("provide approval_id and approved_by for supervised actions");
        }
    }

    let decision_id = prefixed_id("policy");
    let record_id = prefixed_id("bbx_rec");
    let actor = actor_json(input.agent, input.model);
    let payload = json!({
        "record_kind": "policy_decision",
        "policy_decision_id": decision_id,
        "run_id": input.run_id,
        "mode": input.mode.as_str(),
        "status": status,
        "tool_name": input.tool_name,
        "capability_id": input.capability_id,
        "mutating": input.mutating,
        "verifier_id": input.verifier_id,
        "source_scope": input.source_scope,
        "risk_tier": capability.map(|item| item.risk_tier).unwrap_or("unknown"),
        "approval": {
            "approval_id": input.approval_id,
            "approved_by": input.approved_by,
        },
        "error_code": error_code,
        "field_path": field_path,
        "remediation": remediation,
        "evaluated_at": Utc::now().to_rfc3339(),
    });
    let append = append_record(
        &mut conn,
        input.ledger,
        NewRecord {
            record_id: record_id.clone(),
            record_kind: "policy_decision".to_string(),
            tenant_id: input.tenant.to_string(),
            run_id: input.run_id.map(ToOwned::to_owned),
            step_id: None,
            actor,
            payload,
            redaction: RedactionReport::empty(),
        },
    )?;
    Ok(json!({
        "status": "ok",
        "command": "blackbox policy evaluate",
        "decision": status,
        "allowed": status == "allowed",
        "policy_decision_id": decision_id,
        "record_id": record_id,
        "record_kind": "policy_decision",
        "ledger_sequence": append.sequence,
        "source_of_truth": source_of_truth(input.ledger),
        "mode": input.mode.as_str(),
        "tool_name": input.tool_name,
        "capability_id": input.capability_id,
        "mutating": input.mutating,
        "verifier_id": input.verifier_id,
        "error_code": error_code,
        "field_path": field_path,
        "remediation": remediation,
        "payload_sha256": append.payload_sha256,
        "record_sha256": append.record_sha256,
    }))
}

#[derive(Debug, Clone, Copy)]
struct CapabilityDefinition {
    capability_id: &'static str,
    tool_name: &'static str,
    mutating: bool,
    verifier_id: &'static str,
    risk_tier: &'static str,
    source_kind: &'static str,
}

fn capability_registry() -> &'static [CapabilityDefinition] {
    &[
        CapabilityDefinition {
            capability_id: "blackbox.run.record",
            tool_name: "blackbox_start_run",
            mutating: false,
            verifier_id: "ledger_append_integrity_v1",
            risk_tier: "low",
            source_kind: SQLITE_CONNECTOR_ID,
        },
        CapabilityDefinition {
            capability_id: "blackbox.tool.record",
            tool_name: "blackbox_record_tool_call",
            mutating: false,
            verifier_id: "ledger_append_integrity_v1",
            risk_tier: "low",
            source_kind: SQLITE_CONNECTOR_ID,
        },
        CapabilityDefinition {
            capability_id: "blackbox.policy.evaluate",
            tool_name: "blackbox_policy_evaluate",
            mutating: false,
            verifier_id: "ledger_append_integrity_v1",
            risk_tier: "medium",
            source_kind: POLICY_CONNECTOR_ID,
        },
        CapabilityDefinition {
            capability_id: VERIFIED_GIT_APPLY_PATCH_CAPABILITY,
            tool_name: VERIFIED_GIT_APPLY_PATCH_TOOL,
            mutating: true,
            verifier_id: VERIFIED_GIT_APPLY_PATCH_VERIFIER,
            risk_tier: "medium",
            source_kind: GIT_CONNECTOR_ID,
        },
    ]
}

fn capability_by_id(capability_id: &str) -> Option<CapabilityDefinition> {
    capability_registry()
        .iter()
        .copied()
        .find(|item| item.capability_id == capability_id)
}

fn capability_registry_json() -> Value {
    Value::Array(
        capability_registry()
            .iter()
            .map(|item| {
                json!({
                    "capability_id": item.capability_id,
                    "tool_name": item.tool_name,
                    "mutating": item.mutating,
                    "verifier_id": item.verifier_id,
                    "risk_tier": item.risk_tier,
                    "source_kind": item.source_kind,
                    "fail_closed": true,
                })
            })
            .collect(),
    )
}

fn policy_modes_json() -> Value {
    json!([
        { "mode": "observe_only", "blocks": false, "requires_approval": false },
        { "mode": "supervised", "blocks": true, "requires_approval": true },
        { "mode": "bounded_autonomous", "blocks": true, "requires_approval": false },
        { "mode": "blocked", "blocks": true, "requires_approval": false }
    ])
}

fn api_surface(args: ApiSurfaceArgs) -> BbxResult<Value> {
    let ledger_source = if let Some(ledger) = args.ledger {
        let conn = open_initialized_ledger(&ledger)?;
        verify_ledger_integrity(&conn, &ledger)?;
        Value::String(source_of_truth(&ledger))
    } else {
        Value::Null
    };
    Ok(json!({
        "status": "ok",
        "command": "blackbox api surface",
        "source_of_truth": ledger_source,
        "format": "blackbox_http_api_surface_json_v1",
        "endpoints": [
            { "method": "POST", "path": "/v1/runs", "ledger_record_kind": "agent_run" },
            { "method": "GET", "path": "/v1/runs/{run_id}", "ledger_record_kind": "agent_run" },
            { "method": "POST", "path": "/v1/runs/{run_id}/steps", "ledger_record_kind": "agent_step" },
            { "method": "POST", "path": "/v1/tool-calls", "ledger_record_kind": "tool_call" },
            { "method": "POST", "path": "/v1/tool-calls/{tool_call_id}/verify", "ledger_record_kind": "verifier_result" },
            { "method": "GET", "path": "/v1/tool-calls/{tool_call_id}/evidence", "ledger_record_kind": "evidence_packet" },
            { "method": "POST", "path": "/v1/replay", "ledger_record_kind": null },
            { "method": "POST", "path": "/v1/exports/episodes", "ledger_record_kind": null },
            { "method": "GET", "path": "/v1/connectors", "ledger_record_kind": null },
            { "method": "POST", "path": "/v1/connectors/{connector_id}/test", "ledger_record_kind": null },
            { "method": "GET", "path": "/v1/policies", "ledger_record_kind": null },
            { "method": "POST", "path": "/v1/policies/evaluate", "ledger_record_kind": "policy_decision" }
        ],
        "policy_modes": policy_modes_json(),
        "capabilities": capability_registry_json(),
        "fail_closed": true,
    }))
}

#[derive(Debug, Clone)]
struct McpRuntime {
    ledger: PathBuf,
    gateway: bool,
    policy_mode: PolicyMode,
}

async fn handle_mcp_command(action: McpCommands) -> i32 {
    let runtime = match action {
        McpCommands::Recorder(args) => McpRuntime {
            ledger: args.ledger,
            gateway: false,
            policy_mode: PolicyMode::ObserveOnly,
        },
        McpCommands::Gateway(args) => McpRuntime {
            ledger: args.ledger,
            gateway: true,
            policy_mode: args.policy_mode,
        },
    };
    match run_mcp_stdio(runtime).await {
        Ok(()) => 0,
        Err(err) => {
            eprintln!(
                "{}",
                serde_json::to_string_pretty(&err.to_json()).expect("JSON error serializes")
            );
            err.exit_code()
        }
    }
}

async fn run_mcp_stdio(runtime: McpRuntime) -> BbxResult<()> {
    validate_ledger_path(&runtime.ledger)?;
    let conn = open_initialized_ledger(&runtime.ledger)?;
    verify_ledger_integrity(&conn, &runtime.ledger)?;
    drop(conn);

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut writer = BufWriter::new(stdout.lock());
    for line in stdin.lock().lines() {
        let line = line.map_err(|err| {
            BlackboxError::new_with_connector(
                ErrorKind::Io,
                "BBX_MCP_STDIN_READ_FAILED",
                format!("failed to read MCP stdin line: {err}"),
                Some(format!("stdio:{}", runtime.ledger.display())),
                MCP_CONNECTOR_ID,
                Some("stdin".to_string()),
                "send newline-delimited UTF-8 JSON-RPC messages to the MCP server",
            )
        })?;
        let response = handle_mcp_line(&runtime, &line).await?;
        if let Some(response) = response {
            serde_json::to_writer(&mut writer, &response).map_err(|err| {
                BlackboxError::new_with_connector(
                    ErrorKind::InvalidInput,
                    "BBX_MCP_RESPONSE_SERIALIZE_FAILED",
                    format!("failed to serialize MCP response: {err}"),
                    Some(format!("stdio:{}", runtime.ledger.display())),
                    MCP_CONNECTOR_ID,
                    Some("stdout".to_string()),
                    "report this as a bug; MCP responses must be serializable JSON-RPC objects",
                )
            })?;
            writer
                .write_all(b"\n")
                .map_err(|err| BlackboxError::io(&runtime.ledger, "write MCP stdout", err))?;
            writer
                .flush()
                .map_err(|err| BlackboxError::io(&runtime.ledger, "flush MCP stdout", err))?;
        }
    }
    Ok(())
}

async fn handle_mcp_line(runtime: &McpRuntime, line: &str) -> BbxResult<Option<Value>> {
    if line.trim().is_empty() {
        return Ok(Some(mcp_error_response(
            Value::Null,
            -32700,
            "Parse error",
            json!({ "error_code": "BBX_MCP_BLANK_MESSAGE" }),
        )));
    }
    let value: Value = match serde_json::from_str(line) {
        Ok(value) => value,
        Err(err) => {
            return Ok(Some(mcp_error_response(
                Value::Null,
                -32700,
                "Parse error",
                json!({ "error_code": "BBX_MCP_JSON_INVALID", "message": err.to_string() }),
            )));
        }
    };
    if let Some(items) = value.as_array() {
        if items.is_empty() {
            return Ok(Some(mcp_error_response(
                Value::Null,
                -32600,
                "Invalid Request",
                json!({ "error_code": "BBX_MCP_EMPTY_BATCH" }),
            )));
        }
        let mut responses = Vec::new();
        for item in items {
            if let Some(response) = handle_mcp_request(runtime, item).await? {
                responses.push(response);
            }
        }
        if responses.is_empty() {
            Ok(None)
        } else {
            Ok(Some(Value::Array(responses)))
        }
    } else {
        handle_mcp_request(runtime, &value).await
    }
}

async fn handle_mcp_request(runtime: &McpRuntime, request: &Value) -> BbxResult<Option<Value>> {
    let id = request.get("id").cloned().unwrap_or(Value::Null);
    let notification = request.get("id").is_none();
    let Some(object) = request.as_object() else {
        return Ok(Some(mcp_error_response(
            Value::Null,
            -32600,
            "Invalid Request",
            json!({ "error_code": "BBX_MCP_REQUEST_NOT_OBJECT" }),
        )));
    };
    if object.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        return Ok(Some(mcp_error_response(
            id,
            -32600,
            "Invalid Request",
            json!({ "error_code": "BBX_MCP_JSONRPC_VERSION_INVALID" }),
        )));
    }
    let Some(method) = object.get("method").and_then(Value::as_str) else {
        return Ok(Some(mcp_error_response(
            id,
            -32600,
            "Invalid Request",
            json!({ "error_code": "BBX_MCP_METHOD_MISSING" }),
        )));
    };

    let response = match method {
        "initialize" => mcp_success_response(
            id.clone(),
            json!({
                "protocolVersion": MCP_PROTOCOL_VERSION,
                "capabilities": { "tools": { "listChanged": false } },
                "serverInfo": {
                    "name": if runtime.gateway { "blackboxfix-gateway" } else { "blackboxfix-recorder" },
                    "version": env!("CARGO_PKG_VERSION"),
                }
            }),
        ),
        "ping" => mcp_success_response(id.clone(), json!({})),
        "tools/list" => mcp_success_response(
            id.clone(),
            json!({
                "tools": mcp_tool_definitions(runtime.gateway),
                "nextCursor": null,
            }),
        ),
        "tools/call" => match execute_mcp_tool(runtime, object.get("params")).await {
            Ok(result) => mcp_tool_response(id.clone(), result, false),
            Err(err) => mcp_tool_response(id.clone(), err.to_json(), true),
        },
        _ => mcp_error_response(
            id.clone(),
            -32601,
            "Method not found",
            json!({ "error_code": "BBX_MCP_METHOD_NOT_FOUND", "method": method }),
        ),
    };
    if notification {
        Ok(None)
    } else {
        Ok(Some(response))
    }
}

async fn execute_mcp_tool(runtime: &McpRuntime, params: Option<&Value>) -> BbxResult<Value> {
    let params = params.and_then(Value::as_object).ok_or_else(|| {
        mcp_schema_error(
            "params",
            "tools/call params must be an object with name and arguments",
        )
    })?;
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| mcp_schema_error("params.name", "tools/call name is required"))?;
    let arguments = params
        .get("arguments")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            mcp_schema_error("params.arguments", "tools/call arguments must be an object")
        })?;

    match name {
        "blackbox_start_run" => mcp_start_run(runtime, arguments),
        "blackbox_record_decision" => mcp_record_decision(runtime, arguments),
        "blackbox_record_tool_call" => mcp_record_tool_call(runtime, arguments),
        "blackbox_verify_tool_call" => mcp_verify_tool_call(runtime, arguments).await,
        "blackbox_get_evidence" => mcp_get_evidence(runtime, arguments),
        "blackbox_export_episodes" => mcp_export_episodes(runtime, arguments),
        "blackbox_policy_evaluate" => mcp_policy_evaluate(runtime, arguments),
        VERIFIED_GIT_APPLY_PATCH_TOOL if runtime.gateway => {
            mcp_verified_git_apply_patch(runtime, arguments).await
        }
        _ => Err(mcp_schema_error(
            "params.name",
            format!("unknown MCP tool {name}"),
        )),
    }
}

fn mcp_success_response(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn mcp_error_response(id: Value, code: i64, message: &str, data: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message,
            "data": data,
        }
    })
}

fn mcp_tool_response(id: Value, structured: Value, is_error: bool) -> Value {
    let text = serde_json::to_string(&structured).expect("structured MCP tool result serializes");
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "content": [{ "type": "text", "text": text }],
            "structuredContent": structured,
            "isError": is_error,
        }
    })
}

fn mcp_schema_error(field_path: impl Into<String>, message: impl Into<String>) -> BlackboxError {
    BlackboxError::new_with_connector(
        ErrorKind::InvalidInput,
        "BBX_MCP_SCHEMA_INVALID",
        message,
        Some("mcp_stdio:tools/call".to_string()),
        MCP_CONNECTOR_ID,
        Some(field_path.into()),
        "send arguments that match the tool inputSchema advertised by tools/list",
    )
}

fn mcp_required_string(args: &Map<String, Value>, field: &'static str) -> BbxResult<String> {
    args.get(field)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| mcp_schema_error(field, format!("{field} is required")))
}

fn mcp_optional_string(
    args: &Map<String, Value>,
    field: &'static str,
) -> BbxResult<Option<String>> {
    match args.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) if !value.trim().is_empty() => Ok(Some(value.clone())),
        Some(Value::String(_)) => Err(mcp_schema_error(
            field,
            format!("{field} must not be empty"),
        )),
        Some(_) => Err(mcp_schema_error(field, format!("{field} must be a string"))),
    }
}

fn mcp_optional_bool(
    args: &Map<String, Value>,
    field: &'static str,
    default: bool,
) -> BbxResult<bool> {
    match args.get(field) {
        None | Some(Value::Null) => Ok(default),
        Some(Value::Bool(value)) => Ok(*value),
        Some(_) => Err(mcp_schema_error(
            field,
            format!("{field} must be a boolean"),
        )),
    }
}

fn mcp_optional_f64(
    args: &Map<String, Value>,
    field: &'static str,
    default: f64,
) -> BbxResult<f64> {
    match args.get(field) {
        None | Some(Value::Null) => Ok(default),
        Some(Value::Number(value)) => value
            .as_f64()
            .ok_or_else(|| mcp_schema_error(field, format!("{field} must be finite"))),
        Some(_) => Err(mcp_schema_error(field, format!("{field} must be a number"))),
    }
}

fn mcp_object_arg(args: &Map<String, Value>, field: &'static str) -> BbxResult<Value> {
    let value = args
        .get(field)
        .ok_or_else(|| mcp_schema_error(field, format!("{field} is required")))?;
    if !value.is_object() {
        return Err(mcp_schema_error(
            field,
            format!("{field} must be an object"),
        ));
    }
    Ok(value.clone())
}

fn mcp_policy_mode(args: &Map<String, Value>, default: PolicyMode) -> BbxResult<PolicyMode> {
    match args.get("mode") {
        None | Some(Value::Null) => Ok(default),
        Some(Value::String(value)) => match value.as_str() {
            "observe_only" => Ok(PolicyMode::ObserveOnly),
            "supervised" => Ok(PolicyMode::Supervised),
            "bounded_autonomous" => Ok(PolicyMode::BoundedAutonomous),
            "blocked" => Ok(PolicyMode::Blocked),
            _ => Err(mcp_schema_error("mode", "unsupported policy mode")),
        },
        Some(_) => Err(mcp_schema_error("mode", "mode must be a string")),
    }
}

fn mcp_tool_names(include_gateway: bool) -> Vec<String> {
    let mut names = vec![
        "blackbox_start_run".to_string(),
        "blackbox_record_decision".to_string(),
        "blackbox_record_tool_call".to_string(),
        "blackbox_verify_tool_call".to_string(),
        "blackbox_get_evidence".to_string(),
        "blackbox_export_episodes".to_string(),
        "blackbox_policy_evaluate".to_string(),
    ];
    if include_gateway {
        names.push(VERIFIED_GIT_APPLY_PATCH_TOOL.to_string());
    }
    names
}

fn mcp_tool_definitions(include_gateway: bool) -> Vec<Value> {
    let mut tools = vec![
        mcp_tool_definition(
            "blackbox_start_run",
            "BlackBoxFix Start Run",
            "Append an agent_run record to the BlackBoxFix ledger.",
            json_object_schema(
                &[
                    ("task", "string"),
                    ("agent", "string"),
                    ("tenant", "string"),
                    ("model", "string"),
                    ("task_kind", "string"),
                    ("risk_tier", "string"),
                ],
                &["task", "agent"],
            ),
        ),
        mcp_tool_definition(
            "blackbox_record_decision",
            "BlackBoxFix Record Decision",
            "Append agent_step and model_decision rows for a tool choice.",
            json_object_schema(
                &[
                    ("run_id", "string"),
                    ("tool_name", "string"),
                    ("capability_id", "string"),
                    ("input", "object"),
                    ("summary", "string"),
                    ("confidence", "number"),
                    ("tenant", "string"),
                    ("agent", "string"),
                    ("model", "string"),
                ],
                &["run_id", "tool_name", "capability_id"],
            ),
        ),
        mcp_tool_definition(
            "blackbox_record_tool_call",
            "BlackBoxFix Record Tool Call",
            "Append a tool_call record without executing the external tool.",
            json_object_schema(
                &[
                    ("run_id", "string"),
                    ("tool_name", "string"),
                    ("capability_id", "string"),
                    ("input", "object"),
                    ("status", "string"),
                    ("tenant", "string"),
                    ("agent", "string"),
                    ("model", "string"),
                ],
                &["run_id", "tool_name", "capability_id", "input"],
            ),
        ),
        mcp_tool_definition(
            "blackbox_verify_tool_call",
            "BlackBoxFix Verify Tool Call",
            "Run a source-of-truth verifier and append expected/readback/verifier/evidence rows.",
            json_object_schema(
                &[
                    ("spec", "object"),
                    ("tenant", "string"),
                    ("agent", "string"),
                    ("model", "string"),
                ],
                &["spec"],
            ),
        ),
        mcp_tool_definition(
            "blackbox_get_evidence",
            "BlackBoxFix Get Evidence",
            "Read evidence_packet rows from the physical ledger.",
            json_object_schema(
                &[
                    ("run_id", "string"),
                    ("tool_call_id", "string"),
                    ("evidence_packet_id", "string"),
                ],
                &[],
            ),
        ),
        mcp_tool_definition(
            "blackbox_export_episodes",
            "BlackBoxFix Export Episodes",
            "Export verified episodes.jsonl, manifest.json, and eval fixtures.",
            json_object_schema(&[("out", "string"), ("force", "boolean")], &["out"]),
        ),
        mcp_tool_definition(
            "blackbox_policy_evaluate",
            "BlackBoxFix Policy Evaluate",
            "Persist a policy_decision row for one proposed tool action.",
            json_object_schema(
                &[
                    ("mode", "string"),
                    ("run_id", "string"),
                    ("tool_name", "string"),
                    ("capability_id", "string"),
                    ("mutating", "boolean"),
                    ("verifier_id", "string"),
                    ("source_scope", "string"),
                    ("approval_id", "string"),
                    ("approved_by", "string"),
                    ("tenant", "string"),
                    ("agent", "string"),
                    ("model", "string"),
                ],
                &["tool_name", "capability_id", "mutating"],
            ),
        ),
    ];
    if include_gateway {
        tools.push(mcp_tool_definition(
            VERIFIED_GIT_APPLY_PATCH_TOOL,
            "Verified Git Apply Patch",
            "Apply a patch to a real Git repository, then verify the physical diff and persist evidence.",
            json_object_schema(
                &[
                    ("run_id", "string"),
                    ("repo", "string"),
                    ("path", "string"),
                    ("patch", "string"),
                    ("tenant", "string"),
                    ("agent", "string"),
                    ("model", "string"),
                    ("confidence", "number"),
                    ("approval_id", "string"),
                    ("approved_by", "string"),
                ],
                &["run_id", "repo", "path", "patch"],
            ),
        ));
    }
    tools
}

fn mcp_tool_definition(name: &str, title: &str, description: &str, input_schema: Value) -> Value {
    json!({
        "name": name,
        "title": title,
        "description": description,
        "inputSchema": input_schema,
        "outputSchema": {
            "type": "object",
            "additionalProperties": true
        },
        "annotations": {
            "readOnlyHint": name != VERIFIED_GIT_APPLY_PATCH_TOOL,
            "destructiveHint": name == VERIFIED_GIT_APPLY_PATCH_TOOL,
            "idempotentHint": false,
            "openWorldHint": false
        }
    })
}

fn json_object_schema(properties: &[(&str, &str)], required: &[&str]) -> Value {
    let props = properties
        .iter()
        .map(|(name, typ)| ((*name).to_string(), json!({ "type": typ })))
        .collect::<Map<_, _>>();
    json!({
        "type": "object",
        "properties": props,
        "required": required,
        "additionalProperties": false,
    })
}

fn mcp_start_run(runtime: &McpRuntime, args: &Map<String, Value>) -> BbxResult<Value> {
    let task = mcp_required_string(args, "task")?;
    let agent = mcp_required_string(args, "agent")?;
    let tenant = mcp_optional_string(args, "tenant")?.unwrap_or_else(|| "tenant_local".to_string());
    let model = mcp_optional_string(args, "model")?;
    let task_kind =
        mcp_optional_string(args, "task_kind")?.unwrap_or_else(|| "mcp_agent_task".to_string());
    let risk_tier = mcp_optional_string(args, "risk_tier")?.unwrap_or_else(|| "medium".to_string());
    record_run(RecordRunArgs {
        ledger: runtime.ledger.clone(),
        task,
        agent,
        tenant,
        model,
        task_kind,
        risk_tier,
    })
}

fn mcp_record_decision(runtime: &McpRuntime, args: &Map<String, Value>) -> BbxResult<Value> {
    let run_id = mcp_required_string(args, "run_id")?;
    let tool_name = mcp_required_string(args, "tool_name")?;
    let capability_id = mcp_required_string(args, "capability_id")?;
    let tenant = mcp_optional_string(args, "tenant")?.unwrap_or_else(|| "tenant_local".to_string());
    let agent = mcp_optional_string(args, "agent")?.unwrap_or_else(|| "agent_unknown".to_string());
    let model = mcp_optional_string(args, "model")?;
    let confidence = mcp_optional_f64(args, "confidence", 1.0)?;
    let summary = mcp_optional_string(args, "summary")?
        .unwrap_or_else(|| format!("model selected {tool_name}"));
    let input = args
        .get("input")
        .cloned()
        .unwrap_or_else(|| json!({ "tool_name": tool_name, "capability_id": capability_id }));
    if !input.is_object() {
        return Err(mcp_schema_error("input", "input must be an object"));
    }
    let step = record_agent_step_from_parts(
        &runtime.ledger,
        &run_id,
        "model_decision",
        &summary,
        &tenant,
        &agent,
        model.as_deref(),
        json!({ "mcp_tool": "blackbox_record_decision" }),
    )?;
    let step_id = step["step_id"]
        .as_str()
        .expect("record_agent_step output contains step_id");
    let decision = record_model_decision_from_value(
        &runtime.ledger,
        &run_id,
        &tool_name,
        &capability_id,
        confidence,
        &input,
        &tenant,
        &agent,
        model.as_deref(),
        Some(step_id),
    )?;
    Ok(json!({
        "status": "ok",
        "agent_step": step,
        "model_decision": decision,
    }))
}

fn mcp_record_tool_call(runtime: &McpRuntime, args: &Map<String, Value>) -> BbxResult<Value> {
    let run_id = mcp_required_string(args, "run_id")?;
    let tool_name = mcp_required_string(args, "tool_name")?;
    let capability_id = mcp_required_string(args, "capability_id")?;
    let input = mcp_object_arg(args, "input")?;
    let tenant = mcp_optional_string(args, "tenant")?.unwrap_or_else(|| "tenant_local".to_string());
    let agent = mcp_optional_string(args, "agent")?.unwrap_or_else(|| "agent_unknown".to_string());
    let model = mcp_optional_string(args, "model")?;
    let status = match mcp_optional_string(args, "status")?
        .unwrap_or_else(|| "ok".to_string())
        .as_str()
    {
        "ok" => ToolExecutionStatus::Ok,
        "failed" => ToolExecutionStatus::Failed,
        "blocked" => ToolExecutionStatus::Blocked,
        _ => {
            return Err(mcp_schema_error(
                "status",
                "status must be ok, failed, or blocked",
            ))
        }
    };
    record_tool_call_from_value(
        &runtime.ledger,
        &run_id,
        &tool_name,
        &input,
        &tenant,
        &agent,
        model.as_deref(),
        &capability_id,
        status,
        None,
        json!({
            "policy_decision_id": null,
            "mode": "mcp_recorder",
            "approved_by": null,
        }),
        None,
    )
}

async fn mcp_verify_tool_call(runtime: &McpRuntime, args: &Map<String, Value>) -> BbxResult<Value> {
    let spec = mcp_object_arg(args, "spec")?;
    let tenant = mcp_optional_string(args, "tenant")?.unwrap_or_else(|| "tenant_local".to_string());
    let agent =
        mcp_optional_string(args, "agent")?.unwrap_or_else(|| "blackbox_verifier".to_string());
    let model = mcp_optional_string(args, "model")?;
    verify_local_value(&runtime.ledger, &spec, &tenant, &agent, model.as_deref()).await
}

fn mcp_get_evidence(runtime: &McpRuntime, args: &Map<String, Value>) -> BbxResult<Value> {
    let run_id = mcp_optional_string(args, "run_id")?;
    let tool_call_id = mcp_optional_string(args, "tool_call_id")?;
    let evidence_packet_id = mcp_optional_string(args, "evidence_packet_id")?;
    get_evidence_packets(
        &runtime.ledger,
        run_id.as_deref(),
        tool_call_id.as_deref(),
        evidence_packet_id.as_deref(),
    )
}

fn mcp_export_episodes(runtime: &McpRuntime, args: &Map<String, Value>) -> BbxResult<Value> {
    let out = PathBuf::from(mcp_required_string(args, "out")?);
    let force = mcp_optional_bool(args, "force", false)?;
    export_episodes(ExportEpisodesArgs {
        ledger: runtime.ledger.clone(),
        out,
        force,
    })
}

fn mcp_policy_evaluate(runtime: &McpRuntime, args: &Map<String, Value>) -> BbxResult<Value> {
    let mode = mcp_policy_mode(args, runtime.policy_mode)?;
    let run_id = mcp_optional_string(args, "run_id")?;
    let tool_name = mcp_required_string(args, "tool_name")?;
    let capability_id = mcp_required_string(args, "capability_id")?;
    let mutating = mcp_optional_bool(args, "mutating", false)?;
    let verifier_id = mcp_optional_string(args, "verifier_id")?;
    let source_scope = mcp_optional_string(args, "source_scope")?;
    let approval_id = mcp_optional_string(args, "approval_id")?;
    let approved_by = mcp_optional_string(args, "approved_by")?;
    let tenant = mcp_optional_string(args, "tenant")?.unwrap_or_else(|| "tenant_local".to_string());
    let agent = mcp_optional_string(args, "agent")?.unwrap_or_else(|| "agent_unknown".to_string());
    let model = mcp_optional_string(args, "model")?;
    policy_evaluate_from_parts(PolicyEvaluateInput {
        ledger: &runtime.ledger,
        mode,
        run_id: run_id.as_deref(),
        tool_name: &tool_name,
        capability_id: &capability_id,
        mutating,
        verifier_id: verifier_id.as_deref(),
        source_scope: source_scope.as_deref(),
        approval_id: approval_id.as_deref(),
        approved_by: approved_by.as_deref(),
        tenant: &tenant,
        agent: &agent,
        model: model.as_deref(),
    })
}

async fn mcp_verified_git_apply_patch(
    runtime: &McpRuntime,
    args: &Map<String, Value>,
) -> BbxResult<Value> {
    let run_id = mcp_required_string(args, "run_id")?;
    let repo = PathBuf::from(mcp_required_string(args, "repo")?);
    let path = mcp_required_string(args, "path")?;
    let patch = mcp_required_string(args, "patch")?;
    let tenant = mcp_optional_string(args, "tenant")?.unwrap_or_else(|| "tenant_local".to_string());
    let agent = mcp_optional_string(args, "agent")?.unwrap_or_else(|| "agent_unknown".to_string());
    let model = mcp_optional_string(args, "model")?;
    let confidence = mcp_optional_f64(args, "confidence", 1.0)?;
    let approval_id = mcp_optional_string(args, "approval_id")?;
    let approved_by = mcp_optional_string(args, "approved_by")?;
    let source_scope = git_source_of_truth(&repo, &path);

    let step = record_agent_step_from_parts(
        &runtime.ledger,
        &run_id,
        "gateway_tool_execution",
        "gateway accepted verified_git_apply_patch request",
        &tenant,
        &agent,
        model.as_deref(),
        json!({
            "mcp_tool": VERIFIED_GIT_APPLY_PATCH_TOOL,
            "source_scope": source_scope,
        }),
    )?;
    let step_id = step["step_id"]
        .as_str()
        .expect("record_agent_step output contains step_id")
        .to_string();
    let model_decision = record_model_decision_from_value(
        &runtime.ledger,
        &run_id,
        VERIFIED_GIT_APPLY_PATCH_TOOL,
        VERIFIED_GIT_APPLY_PATCH_CAPABILITY,
        confidence,
        &json!({
            "policy_context": {
                "mode": runtime.policy_mode.as_str(),
                "source_scope": source_scope,
            },
            "tool_input": {
                "repo": repo.display().to_string(),
                "path": path.clone(),
                "patch_sha256": sha256_hex(patch.as_bytes()),
            }
        }),
        &tenant,
        &agent,
        model.as_deref(),
        Some(&step_id),
    )?;
    let policy = policy_evaluate_from_parts(PolicyEvaluateInput {
        ledger: &runtime.ledger,
        mode: runtime.policy_mode,
        run_id: Some(&run_id),
        tool_name: VERIFIED_GIT_APPLY_PATCH_TOOL,
        capability_id: VERIFIED_GIT_APPLY_PATCH_CAPABILITY,
        mutating: true,
        verifier_id: Some(VERIFIED_GIT_APPLY_PATCH_VERIFIER),
        source_scope: Some(&source_scope),
        approval_id: approval_id.as_deref(),
        approved_by: approved_by.as_deref(),
        tenant: &tenant,
        agent: &agent,
        model: model.as_deref(),
    })?;
    if policy["allowed"].as_bool() != Some(true) {
        return Err(BlackboxError::connector_blocked(
            static_policy_error_code(policy["error_code"].as_str()),
            POLICY_CONNECTOR_ID,
            source_of_truth(&runtime.ledger),
            "policy_decision.status",
            format!(
                "policy denied {} for capability {}",
                VERIFIED_GIT_APPLY_PATCH_TOOL, VERIFIED_GIT_APPLY_PATCH_CAPABILITY
            ),
            policy["remediation"]
                .as_str()
                .unwrap_or("inspect the persisted policy_decision row and fix the policy inputs"),
        ));
    }

    let pre_diff = run_git(&repo, &["diff", "--no-ext-diff", "--no-color", "--", &path])?;
    if pre_diff.exit_code != 0 {
        return Err(BlackboxError::connector_blocked(
            "GATEWAY_GIT_PRESTATE_READ_FAILED",
            GIT_CONNECTOR_ID,
            source_scope.clone(),
            "git.diff",
            "failed to read pre-state git diff before applying patch",
            "inspect git stderr and provide a valid repository/path before retrying",
        ));
    }
    if !pre_diff.stdout.trim().is_empty() {
        return Err(BlackboxError::connector_blocked(
            "GATEWAY_GIT_PRESTATE_DIRTY",
            GIT_CONNECTOR_ID,
            source_scope.clone(),
            "git.diff",
            "target path already has uncommitted changes before gateway execution",
            "start from a clean target path or split the prior change into its own verified action",
        ));
    }
    let check = run_git_with_stdin(&repo, &["apply", "--check", "-"], &patch)?;
    if check.exit_code != 0 {
        return Err(BlackboxError::connector_blocked(
            "GATEWAY_GIT_APPLY_CHECK_FAILED",
            GIT_CONNECTOR_ID,
            source_scope.clone(),
            "patch",
            format!("git apply --check failed: {}", check.stderr),
            "provide a patch that applies cleanly to the current physical repository state",
        ));
    }

    let tool_input = json!({
        "repo": repo.display().to_string(),
        "path": path.clone(),
        "patch": patch.clone(),
    });
    let tool_call = record_tool_call_from_value(
        &runtime.ledger,
        &run_id,
        VERIFIED_GIT_APPLY_PATCH_TOOL,
        &tool_input,
        &tenant,
        &agent,
        model.as_deref(),
        VERIFIED_GIT_APPLY_PATCH_CAPABILITY,
        ToolExecutionStatus::Ok,
        None,
        json!({
            "policy_decision_id": policy["policy_decision_id"],
            "policy_decision_sequence": policy["ledger_sequence"],
            "mode": runtime.policy_mode.as_str(),
            "approved_by": approved_by,
        }),
        Some(&step_id),
    )?;
    let tool_call_id = tool_call["tool_call_id"]
        .as_str()
        .expect("record_tool_call output contains tool_call_id")
        .to_string();

    let apply = run_git_with_stdin(&repo, &["apply", "-"], &patch)?;
    let verifier_spec = json!({
        "verifier_id": VERIFIED_GIT_APPLY_PATCH_VERIFIER,
        "run_id": run_id,
        "tool_call_id": tool_call_id,
        "source": { "repo": repo.display().to_string() },
        "expected": { "path": path, "patch": patch }
    });
    let verification = verify_local_value(
        &runtime.ledger,
        &verifier_spec,
        &tenant,
        &agent,
        model.as_deref(),
    )
    .await;
    let evidence =
        get_evidence_packets(&runtime.ledger, Some(&run_id), Some(&tool_call_id), None).ok();
    if apply.exit_code != 0 {
        return Err(BlackboxError::connector_blocked(
            "GATEWAY_GIT_APPLY_FAILED",
            GIT_CONNECTOR_ID,
            source_scope.clone(),
            "git.apply",
            format!("git apply failed after preflight: {}", apply.stderr),
            "inspect the physical repository state; the verifier evidence row records the post-failure readback when available",
        ));
    }
    let verification = verification?;
    Ok(json!({
        "status": "ok",
        "command": VERIFIED_GIT_APPLY_PATCH_TOOL,
        "source_of_truth": source_scope,
        "agent_step": step,
        "model_decision": model_decision,
        "policy_decision": policy,
        "tool_call": tool_call,
        "verification": verification,
        "evidence": evidence,
    }))
}

fn static_policy_error_code(error_code: Option<&str>) -> &'static str {
    match error_code {
        Some("POLICY_MODE_BLOCKED") => "POLICY_MODE_BLOCKED",
        Some("POLICY_CAPABILITY_UNKNOWN") => "POLICY_CAPABILITY_UNKNOWN",
        Some("POLICY_VERIFIER_REQUIRED") => "POLICY_VERIFIER_REQUIRED",
        Some("POLICY_CAPABILITY_MUTATION_MISMATCH") => "POLICY_CAPABILITY_MUTATION_MISMATCH",
        Some("POLICY_VERIFIER_MISMATCH") => "POLICY_VERIFIER_MISMATCH",
        Some("POLICY_APPROVAL_REQUIRED") => "POLICY_APPROVAL_REQUIRED",
        _ => "POLICY_DENIED",
    }
}

fn get_evidence_packets(
    ledger: &Path,
    run_id: Option<&str>,
    tool_call_id: Option<&str>,
    evidence_packet_id: Option<&str>,
) -> BbxResult<Value> {
    validate_ledger_path(ledger)?;
    let conn = open_initialized_ledger(ledger)?;
    let inspection = verify_ledger_integrity(&conn, ledger)?;
    let packets = inspection
        .records
        .into_iter()
        .filter(|record| record.record_kind == "evidence_packet")
        .filter(|record| {
            run_id
                .map(|expected| record.run_id.as_deref() == Some(expected))
                .unwrap_or(true)
        })
        .filter(|record| {
            tool_call_id
                .map(|expected| {
                    record.payload.get("tool_call_id").and_then(Value::as_str) == Some(expected)
                })
                .unwrap_or(true)
        })
        .filter(|record| {
            evidence_packet_id
                .map(|expected| {
                    record
                        .payload
                        .get("evidence_packet_id")
                        .and_then(Value::as_str)
                        == Some(expected)
                })
                .unwrap_or(true)
        })
        .collect::<Vec<_>>();
    Ok(json!({
        "status": "ok",
        "command": "blackbox_get_evidence",
        "source_of_truth": source_of_truth(ledger),
        "count": packets.len(),
        "evidence_packets": packets,
    }))
}

fn ledger_record_ref_by_run(
    conn: &Connection,
    path: &Path,
    run_id: &str,
    role: &str,
) -> BbxResult<Value> {
    let found: Option<(i64, String, String, String, String)> = conn
        .query_row(
            "SELECT sequence, record_id, record_kind, payload_sha256, record_sha256
             FROM ledger_records
             WHERE record_kind = 'agent_run' AND run_id = ?1
             ORDER BY sequence ASC
             LIMIT 1",
            params![run_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .optional()
        .map_err(|err| BlackboxError::sqlite(path, "read agent_run reference", err))?;
    let Some((sequence, record_id, record_kind, payload_sha256, record_sha256)) = found else {
        return Err(BlackboxError::not_found(
            path,
            format!("run_id {run_id} does not exist in ledger_records"),
        ));
    };
    Ok(ledger_record_ref_json(
        role,
        sequence,
        &record_id,
        &record_kind,
        &payload_sha256,
        &record_sha256,
    ))
}

fn ledger_record_ref_by_tool_call(
    conn: &Connection,
    path: &Path,
    tool_call_id: &str,
    role: &str,
) -> BbxResult<Value> {
    let found: Option<(i64, String, String, String, String)> = conn
        .query_row(
            "SELECT sequence, record_id, record_kind, payload_sha256, record_sha256
             FROM ledger_records
             WHERE record_kind = 'tool_call'
               AND json_extract(payload_json, '$.tool_call_id') = ?1
             ORDER BY sequence ASC
             LIMIT 1",
            params![tool_call_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .optional()
        .map_err(|err| BlackboxError::sqlite(path, "read tool_call reference", err))?;
    let Some((sequence, record_id, record_kind, payload_sha256, record_sha256)) = found else {
        return Err(BlackboxError::not_found(
            path,
            format!("tool_call_id {tool_call_id} does not exist in ledger_records"),
        ));
    };
    Ok(ledger_record_ref_json(
        role,
        sequence,
        &record_id,
        &record_kind,
        &payload_sha256,
        &record_sha256,
    ))
}

fn ledger_record_ref_json(
    role: &str,
    sequence: i64,
    record_id: &str,
    record_kind: &str,
    payload_sha256: &str,
    record_sha256: &str,
) -> Value {
    json!({
        "role": role,
        "sequence": sequence,
        "record_id": record_id,
        "record_kind": record_kind,
        "payload_sha256": payload_sha256,
        "record_sha256": record_sha256,
    })
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
    verify_local_value(
        &args.ledger,
        &spec_value,
        &args.tenant,
        &args.agent,
        args.model.as_deref(),
    )
    .await
}

async fn verify_local_value(
    ledger: &Path,
    spec_value: &Value,
    tenant: &str,
    agent: &str,
    model: Option<&str>,
) -> BbxResult<Value> {
    let redacted_spec = redact_tool_input(ledger, spec_value)?;
    let spec = VerifySpec::parse(ledger, spec_value)?;

    let mut conn = open_initialized_ledger(ledger)?;
    ensure_tool_call_exists_for_run_tenant(
        &conn,
        ledger,
        &spec.run_id,
        &spec.tool_call_id,
        tenant,
    )?;

    let run_id = spec.run_id.clone();
    let tool_call_id = spec.tool_call_id.clone();
    let verifier_id = spec.verifier_id.clone();
    let expected_value = spec.expected.clone();
    let tenant_id = tenant.to_string();
    let actor = actor_json(agent, model);
    let expected_delta_id = prefixed_id("expected");
    let expected_record_id = prefixed_id("bbx_rec");
    let expected_payload = json!({
        "record_kind": "expected_delta",
        "expected_delta_id": expected_delta_id,
        "tool_call_id": tool_call_id,
        "verifier_id": verifier_id,
        "source_of_truth": spec.source_of_truth_hint(),
        "expected_persisted_delta": expected_value,
        "spec_redacted": redacted_spec.value,
        "recorded_at": Utc::now().to_rfc3339(),
    });
    let expected_append = append_record(
        &mut conn,
        ledger,
        NewRecord {
            record_id: expected_record_id.clone(),
            record_kind: "expected_delta".to_string(),
            tenant_id: tenant_id.clone(),
            run_id: Some(run_id.clone()),
            step_id: None,
            actor: actor.clone(),
            payload: expected_payload,
            redaction: redacted_spec.report,
        },
    )?;

    let verification = run_phase2_verifier(&spec).await;
    let mut readback_record_id: Option<String> = None;
    let readback_append = if let Some(readback_payload) = verification.readback_payload.clone() {
        let generated_readback_record_id = prefixed_id("bbx_rec");
        readback_record_id = Some(generated_readback_record_id.clone());
        Some(append_record(
            &mut conn,
            ledger,
            NewRecord {
                record_id: generated_readback_record_id,
                record_kind: "readback_snapshot".to_string(),
                tenant_id: tenant_id.clone(),
                run_id: Some(run_id.clone()),
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
    let verifier_record_id = prefixed_id("bbx_rec");
    let verifier_payload = json!({
        "record_kind": "verifier_result",
        "verifier_result_id": verifier_result_id,
        "tool_call_id": tool_call_id,
        "verifier_id": verifier_id,
        "status": verification.status,
        "expected_persisted_delta": expected_value,
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
        ledger,
        NewRecord {
            record_id: verifier_record_id.clone(),
            record_kind: "verifier_result".to_string(),
            tenant_id: tenant_id.clone(),
            run_id: Some(run_id.clone()),
            step_id: None,
            actor: actor.clone(),
            payload: verifier_payload,
            redaction: RedactionReport::empty(),
        },
    )?;

    let run_ref = ledger_record_ref_by_run(&conn, ledger, &run_id, "run")?;
    let tool_call_ref = ledger_record_ref_by_tool_call(&conn, ledger, &tool_call_id, "tool_call")?;
    let expected_ref = ledger_record_ref_json(
        "expected_delta",
        expected_append.sequence,
        &expected_record_id,
        "expected_delta",
        &expected_append.payload_sha256,
        &expected_append.record_sha256,
    );
    let readback_ref = readback_append.as_ref().map(|append| {
        let record_id = readback_record_id
            .as_deref()
            .expect("readback record id exists when readback append exists");
        ledger_record_ref_json(
            "readback_snapshot",
            append.sequence,
            record_id,
            "readback_snapshot",
            &append.payload_sha256,
            &append.record_sha256,
        )
    });
    let verifier_ref = ledger_record_ref_json(
        "verifier_result",
        verifier_append.sequence,
        &verifier_record_id,
        "verifier_result",
        &verifier_append.payload_sha256,
        &verifier_append.record_sha256,
    );
    let mut record_refs = vec![run_ref, tool_call_ref, expected_ref];
    if let Some(readback_ref) = &readback_ref {
        record_refs.push(readback_ref.clone());
    }
    record_refs.push(verifier_ref);

    let actual_readback_sha256 = sha256_hex(&canonical_json_bytes(&verification.actual_readback)?);
    let expected_delta_sha256 = sha256_hex(&canonical_json_bytes(&expected_value)?);
    let evidence_packet_id = prefixed_id("ev");
    let evidence_payload = json!({
        "record_kind": "evidence_packet",
        "evidence_packet_id": evidence_packet_id,
        "run_id": run_id,
        "tool_call_id": tool_call_id,
        "verifier_result_id": verifier_result_id,
        "verifier_id": verifier_id,
        "verifier_status": verification.status,
        "source_of_truth": [{
            "kind": verification.connector,
            "locator": verification.source_of_truth,
            "readback_sequence": readback_append.as_ref().map(|append| append.sequence),
            "readback_record_sha256": readback_append.as_ref().map(|append| append.record_sha256.clone()),
            "state_hash": actual_readback_sha256,
        }],
        "provenance": {
            "model": "w3c-prov-inspired",
            "entities": record_refs,
            "activities": [{
                "activity_kind": "blackbox_verify",
                "verifier_id": verifier_id,
                "used": ["run", "tool_call", "expected_delta", "readback_snapshot"],
                "generated": ["verifier_result", "evidence_packet"],
            }],
            "agents": [actor],
        },
        "record_refs": record_refs,
        "hashes": {
            "expected_delta_sha256": expected_delta_sha256,
            "actual_readback_sha256": actual_readback_sha256,
            "expected_delta_record_sha256": expected_append.record_sha256,
            "readback_snapshot_record_sha256": readback_append.as_ref().map(|append| append.record_sha256.clone()),
            "verifier_result_record_sha256": verifier_append.record_sha256,
        },
        "created_at": Utc::now().to_rfc3339(),
    });
    let evidence_record_id = prefixed_id("bbx_rec");
    let evidence_append = append_record(
        &mut conn,
        ledger,
        NewRecord {
            record_id: evidence_record_id,
            record_kind: "evidence_packet".to_string(),
            tenant_id: tenant_id.clone(),
            run_id: Some(run_id.clone()),
            step_id: None,
            actor: actor.clone(),
            payload: evidence_payload,
            redaction: RedactionReport::empty(),
        },
    )?;

    let incident_append = if verification.status == "failed" {
        let incident_packet_id = prefixed_id("inc");
        let incident_payload = json!({
            "record_kind": "incident_packet",
            "incident_packet_id": incident_packet_id,
            "severity": "high",
            "run_id": run_id,
            "tool_call_id": tool_call_id,
            "trigger": "verifier_failed",
            "verifier_id": verifier_id,
            "verifier_result_id": verifier_result_id,
            "evidence_packet_id": evidence_packet_id,
            "evidence_packet_sequence": evidence_append.sequence,
            "evidence_packet_record_sha256": evidence_append.record_sha256,
            "error_code": verification.error_code,
            "error_message": verification.message,
            "field_path": verification.field_path,
            "source_of_truth": [verification.source_of_truth],
            "blast_radius": {
                "sources": [verification.source_of_truth],
                "repos": [],
                "tickets": [],
                "deployments": [],
            },
            "remediation": verification.remediation,
            "timeline": record_refs,
            "created_at": Utc::now().to_rfc3339(),
        });
        Some(append_record(
            &mut conn,
            ledger,
            NewRecord {
                record_id: prefixed_id("bbx_rec"),
                record_kind: "incident_packet".to_string(),
                tenant_id: tenant_id.clone(),
                run_id: Some(run_id.clone()),
                step_id: None,
                actor: actor.clone(),
                payload: incident_payload,
                redaction: RedactionReport::empty(),
            },
        )?)
    } else {
        None
    };

    let output = json!({
        "status": "ok",
        "command": "blackbox verify",
        "verifier_id": verifier_id,
        "verifier_status": verification.status,
        "connector": verification.connector,
        "source_of_truth": verification.source_of_truth,
        "expected_delta_sequence": expected_append.sequence,
        "readback_snapshot_sequence": readback_append.as_ref().map(|append| append.sequence),
        "verifier_result_sequence": verifier_append.sequence,
        "expected_delta_record_sha256": expected_append.record_sha256,
        "readback_snapshot_record_sha256": readback_append.as_ref().map(|append| append.record_sha256.clone()),
        "verifier_result_record_sha256": verifier_append.record_sha256,
        "evidence_packet_id": evidence_packet_id,
        "evidence_packet_sequence": evidence_append.sequence,
        "evidence_packet_record_sha256": evidence_append.record_sha256,
        "incident_packet_sequence": incident_append.as_ref().map(|append| append.sequence),
        "incident_packet_record_sha256": incident_append.as_ref().map(|append| append.record_sha256.clone()),
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

fn replay_run(args: ReplayArgs) -> BbxResult<Value> {
    validate_ledger_path(&args.ledger)?;
    require_non_empty_for_ledger(&args.ledger, "--run-id", &args.run_id)?;
    let conn = open_initialized_ledger(&args.ledger)?;
    let inspection = verify_ledger_integrity(&conn, &args.ledger)?;
    let report = build_run_replay_report(&args.ledger, &inspection, &args.run_id)?;
    Ok(json!({
        "status": "ok",
        "command": "blackbox replay",
        "source_of_truth": source_of_truth(&args.ledger),
        "run_id": args.run_id,
        "report": report,
    }))
}

fn export_evidence(args: ExportEvidenceArgs) -> BbxResult<Value> {
    validate_ledger_path(&args.ledger)?;
    require_non_empty_for_ledger(&args.ledger, "--run-id", &args.run_id)?;
    validate_export_output_path(&args.out)?;

    let conn = open_initialized_ledger(&args.ledger)?;
    let inspection = verify_ledger_integrity(&conn, &args.ledger)?;
    let report = build_run_replay_report(&args.ledger, &inspection, &args.run_id)?;
    let export = json!({
        "format": "blackbox_evidence_report_json_v1",
        "pdf_ready": true,
        "generated_at": Utc::now().to_rfc3339(),
        "source_of_truth": source_of_truth(&args.ledger),
        "report": report,
    });
    let bytes = serde_json::to_vec_pretty(&export).map_err(|err| {
        BlackboxError::new(
            ErrorKind::InvalidInput,
            "EVIDENCE_EXPORT_SERIALIZE_FAILED",
            format!("failed to serialize evidence export JSON: {err}"),
            Some(source_of_truth(&args.ledger)),
            Some("export".to_string()),
            "inspect non-serializable export fields before retrying",
        )
    })?;
    fs::write(&args.out, &bytes)
        .map_err(|err| BlackboxError::io(&args.out, "write evidence export JSON", err))?;
    let readback = fs::read(&args.out)
        .map_err(|err| BlackboxError::io(&args.out, "read evidence export JSON", err))?;
    if readback != bytes {
        return Err(BlackboxError::evidence_integrity_for_connector(
            "EVIDENCE_EXPORT_READBACK_MISMATCH",
            FS_CONNECTOR_ID,
            file_source_of_truth(&args.out),
            "--out",
            "evidence export readback bytes do not match bytes written",
            "treat the export target as unreliable; inspect filesystem/storage before retrying",
        ));
    }
    let parsed: Value = serde_json::from_slice(&readback).map_err(|err| {
        BlackboxError::evidence_integrity_for_connector(
            "EVIDENCE_EXPORT_JSON_INVALID",
            FS_CONNECTOR_ID,
            file_source_of_truth(&args.out),
            "$",
            format!("evidence export file does not parse as JSON after write: {err}"),
            "delete the corrupt export file and rerun after fixing the filesystem/storage path",
        )
    })?;
    Ok(json!({
        "status": "ok",
        "command": "blackbox export evidence",
        "source_of_truth": file_source_of_truth(&args.out),
        "ledger_source_of_truth": source_of_truth(&args.ledger),
        "run_id": args.run_id,
        "out": args.out,
        "size_bytes": readback.len(),
        "sha256": sha256_hex(&readback),
        "record_count": parsed.pointer("/report/summary/record_count").cloned().unwrap_or(Value::Null),
        "evidence_packet_count": parsed.pointer("/report/summary/kinds/evidence_packet").cloned().unwrap_or(Value::Null),
        "incident_packet_count": parsed.pointer("/report/summary/kinds/incident_packet").cloned().unwrap_or(Value::Null),
    }))
}

fn export_episodes(args: ExportEpisodesArgs) -> BbxResult<Value> {
    validate_ledger_path(&args.ledger)?;
    validate_export_directory_path(&args.out, args.force)?;

    let conn = open_initialized_ledger(&args.ledger)?;
    let inspection = verify_ledger_integrity(&conn, &args.ledger)?;
    let ledger_id = get_metadata(&conn, &args.ledger, "ledger_id")?.unwrap_or_default();
    validate_tool_calls_have_evidence(&args.ledger, &inspection.records)?;
    let record_refs = inspection.records.iter().collect::<Vec<_>>();
    verify_evidence_packet_refs(&args.ledger, &record_refs)?;

    let rows = build_episode_rows(&args.ledger, &inspection)?;
    if rows.is_empty() {
        return Err(BlackboxError::new_with_connector(
            ErrorKind::InvalidInput,
            "BBX_NO_EXPORTABLE_EPISODES",
            "ledger contains no evidence_packet rows to export",
            Some(source_of_truth(&args.ledger)),
            SQLITE_CONNECTOR_ID,
            Some("ledger_records.record_kind".to_string()),
            "run one or more verifiers so the ledger contains durable evidence_packet rows",
        ));
    }

    let temp_dir = export_temp_dir(&args.out);
    cleanup_export_temp_dir(&temp_dir)?;
    let export_result =
        write_episode_export_directory(&args, &inspection, &ledger_id, &rows, &temp_dir);
    match export_result {
        Ok(summary) => Ok(summary),
        Err(err) => {
            let _ = fs::remove_dir_all(&temp_dir);
            Err(err)
        }
    }
}

fn dashboard_api(args: DashboardApiArgs) -> BbxResult<Value> {
    validate_ledger_path(&args.ledger)?;
    if let Some(run_id) = &args.run_id {
        require_non_empty_for_ledger(&args.ledger, "--run-id", run_id)?;
    }

    let conn = open_initialized_ledger(&args.ledger)?;
    let inspection = verify_ledger_integrity(&conn, &args.ledger)?;
    let ledger_id = get_metadata(&conn, &args.ledger, "ledger_id")?.unwrap_or_default();
    let data = build_dashboard_data(&args.ledger, &inspection, &ledger_id)?;
    let view = match args.view {
        DashboardView::Summary => data.summary,
        DashboardView::Runs => data.runs,
        DashboardView::Evidence => data.evidence,
        DashboardView::Verifiers => data.verifiers,
        DashboardView::Errors => data.errors,
        DashboardView::Exports => data.exports,
        DashboardView::Timeline => {
            let run_id = args.run_id.as_deref().ok_or_else(|| {
                BlackboxError::new_with_connector(
                    ErrorKind::InvalidInput,
                    "DASHBOARD_RUN_ID_REQUIRED",
                    "dashboard timeline API requires --run-id",
                    Some(source_of_truth(&args.ledger)),
                    SQLITE_CONNECTOR_ID,
                    Some("--run-id".to_string()),
                    "provide the run_id from `blackbox dashboard api --view runs`",
                )
            })?;
            data.timelines
                .into_iter()
                .find(|(candidate, _, _)| candidate == run_id)
                .map(|(_, _, timeline)| timeline)
                .ok_or_else(|| {
                    BlackboxError::not_found(
                        &args.ledger,
                        format!("run_id {run_id} does not exist in dashboard timelines"),
                    )
                })?
        }
    };
    Ok(json!({
        "status": "ok",
        "command": "blackbox dashboard api",
        "view": args.view.as_str(),
        "source_of_truth": source_of_truth(&args.ledger),
        "dashboard_schema_version": DASHBOARD_SCHEMA_VERSION,
        "generated_at": data.generated_at,
        "data": view,
    }))
}

fn dashboard_build(args: DashboardBuildArgs) -> BbxResult<Value> {
    validate_ledger_path(&args.ledger)?;
    validate_dashboard_directory_path(&args.out, args.force)?;

    let conn = open_initialized_ledger(&args.ledger)?;
    let inspection = verify_ledger_integrity(&conn, &args.ledger)?;
    let ledger_id = get_metadata(&conn, &args.ledger, "ledger_id")?.unwrap_or_default();
    let data = build_dashboard_data(&args.ledger, &inspection, &ledger_id)?;

    let temp_dir = export_temp_dir(&args.out);
    cleanup_dashboard_temp_dir(&temp_dir)?;
    let build_result = write_dashboard_directory(&args, &inspection, &data, &temp_dir);
    match build_result {
        Ok(summary) => Ok(summary),
        Err(err) => {
            let _ = fs::remove_dir_all(&temp_dir);
            Err(err)
        }
    }
}

#[derive(Debug)]
struct DashboardData {
    generated_at: String,
    summary: Value,
    runs: Value,
    evidence: Value,
    verifiers: Value,
    errors: Value,
    exports: Value,
    timelines: Vec<(String, String, Value)>,
}

fn build_dashboard_data(
    ledger_path: &Path,
    inspection: &LedgerInspection,
    ledger_id: &str,
) -> BbxResult<DashboardData> {
    let generated_at = Utc::now().to_rfc3339();
    let run_records = inspection
        .records
        .iter()
        .filter(|record| record.record_kind == "agent_run")
        .collect::<Vec<_>>();
    if run_records.is_empty() {
        return Err(BlackboxError::new_with_connector(
            ErrorKind::InvalidInput,
            "DASHBOARD_NO_RUNS",
            "ledger contains no agent_run rows for the dashboard",
            Some(source_of_truth(ledger_path)),
            SQLITE_CONNECTOR_ID,
            Some("ledger_records.record_kind".to_string()),
            "record at least one agent_run before building the dashboard",
        ));
    }

    let all_record_refs = inspection.records.iter().collect::<Vec<_>>();
    let evidence_hash_verification = verify_evidence_packet_refs(ledger_path, &all_record_refs)?;
    let kind_counts = ledger_record_kind_counts(&inspection.records);
    let records_by_run = records_by_run_id(&inspection.records);
    let tool_call_by_id = tool_call_records_by_id(ledger_path, &inspection.records)?;
    let evidence_tool_call_ids = evidence_tool_call_ids(ledger_path, &inspection.records)?;
    let summary = build_dashboard_summary(
        ledger_path,
        inspection,
        ledger_id,
        &generated_at,
        &kind_counts,
        &evidence_hash_verification,
        &evidence_tool_call_ids,
    )?;
    let runs = build_dashboard_runs(
        ledger_path,
        inspection,
        ledger_id,
        &generated_at,
        &run_records,
        &records_by_run,
    )?;
    let evidence = build_dashboard_evidence(
        ledger_path,
        inspection,
        ledger_id,
        &generated_at,
        &inspection.records,
    )?;
    let verifiers = build_dashboard_verifiers(
        ledger_path,
        inspection,
        ledger_id,
        &generated_at,
        &inspection.records,
        &tool_call_by_id,
    )?;
    let errors = build_dashboard_errors(
        ledger_path,
        inspection,
        ledger_id,
        &generated_at,
        &inspection.records,
        &tool_call_by_id,
    )?;
    let exports = build_dashboard_exports(
        ledger_path,
        inspection,
        ledger_id,
        &generated_at,
        &run_records,
    )?;
    let timelines = build_dashboard_timelines(ledger_path, inspection, &run_records)?;
    Ok(DashboardData {
        generated_at,
        summary,
        runs,
        evidence,
        verifiers,
        errors,
        exports,
        timelines,
    })
}

fn write_dashboard_directory(
    args: &DashboardBuildArgs,
    inspection: &LedgerInspection,
    data: &DashboardData,
    temp_dir: &Path,
) -> BbxResult<Value> {
    fs::create_dir(temp_dir)
        .map_err(|err| BlackboxError::io(temp_dir, "create dashboard temp directory", err))?;
    let api_dir = temp_dir.join("api");
    fs::create_dir(&api_dir)
        .map_err(|err| BlackboxError::io(&api_dir, "create dashboard api directory", err))?;

    write_dashboard_json_file(&api_dir.join("summary.json"), &data.summary)?;
    write_dashboard_json_file(&api_dir.join("runs.json"), &data.runs)?;
    write_dashboard_json_file(&api_dir.join("evidence.json"), &data.evidence)?;
    write_dashboard_json_file(&api_dir.join("verifier-status.json"), &data.verifiers)?;
    write_dashboard_json_file(&api_dir.join("errors.json"), &data.errors)?;
    write_dashboard_json_file(&api_dir.join("exports.json"), &data.exports)?;

    let mut timeline_files = BTreeSet::new();
    for (_, file_name, timeline) in &data.timelines {
        if !timeline_files.insert(file_name.clone()) {
            return Err(BlackboxError::evidence_integrity_for_connector(
                "DASHBOARD_TIMELINE_FILENAME_COLLISION",
                FS_CONNECTOR_ID,
                file_source_of_truth(temp_dir),
                "api/timeline_*.json",
                format!("dashboard timeline filename collision for {file_name}"),
                "use distinct run identifiers before building the dashboard",
            ));
        }
        write_dashboard_json_file(&api_dir.join(file_name), timeline)?;
    }

    let html = build_dashboard_html(data)?;
    write_dashboard_text_file(&temp_dir.join("index.html"), &html, "dashboard HTML")?;

    let mut files = vec![
        dashboard_file_entry(temp_dir, Path::new("index.html"))?,
        dashboard_file_entry(temp_dir, Path::new("api/summary.json"))?,
        dashboard_file_entry(temp_dir, Path::new("api/runs.json"))?,
        dashboard_file_entry(temp_dir, Path::new("api/evidence.json"))?,
        dashboard_file_entry(temp_dir, Path::new("api/verifier-status.json"))?,
        dashboard_file_entry(temp_dir, Path::new("api/errors.json"))?,
        dashboard_file_entry(temp_dir, Path::new("api/exports.json"))?,
    ];
    for (_, file_name, _) in &data.timelines {
        files.push(dashboard_file_entry(
            temp_dir,
            &Path::new("api").join(file_name),
        )?);
    }

    let manifest = json!({
        "status": "ok",
        "format": "blackbox_dashboard_manifest_json_v1",
        "dashboard_schema_version": DASHBOARD_SCHEMA_VERSION,
        "generated_at": data.generated_at,
        "source_ledger": {
            "path": args.ledger,
            "source_of_truth": source_of_truth(&args.ledger),
            "ledger_id": data.summary["source_ledger"]["ledger_id"].clone(),
            "ledger_schema_version": LEDGER_SCHEMA_VERSION,
            "ledger_record_count": inspection.record_count,
            "ledger_tip_sha256_at_build": inspection.ledger_tip_sha256,
        },
        "output": {
            "directory": args.out,
            "source_of_truth": file_source_of_truth(&args.out),
            "api_directory": "api",
            "index": "index.html",
        },
        "counts": {
            "runs_total": data.summary["counts"]["runs_total"].clone(),
            "evidence_packets_total": data.summary["counts"]["evidence_packets_total"].clone(),
            "incident_packets_total": data.summary["counts"]["incident_packets_total"].clone(),
            "timeline_files_total": data.timelines.len(),
            "files_total": files.len() + 1,
        },
        "files": files,
    });
    write_dashboard_json_file(&temp_dir.join("manifest.json"), &manifest)?;
    promote_dashboard_dir(temp_dir, &args.out, args.force)?;

    let final_manifest = args.out.join("manifest.json");
    let final_manifest_json = read_json_file(&final_manifest, "final dashboard manifest")?;
    if final_manifest_json["format"] != "blackbox_dashboard_manifest_json_v1" {
        return Err(BlackboxError::evidence_integrity_for_connector(
            "DASHBOARD_MANIFEST_READBACK_MISMATCH",
            FS_CONNECTOR_ID,
            file_source_of_truth(&final_manifest),
            "manifest.json.format",
            "final dashboard manifest readback did not preserve the expected format",
            "delete the corrupt dashboard directory and rerun after inspecting filesystem/storage consistency",
        ));
    }
    verify_dashboard_manifest_files(&args.out, &final_manifest_json)?;
    let manifest_sha256 = sha256_file(&final_manifest)?;

    Ok(json!({
        "status": "ok",
        "command": "blackbox dashboard build",
        "source_of_truth": file_source_of_truth(&args.out),
        "ledger_source_of_truth": source_of_truth(&args.ledger),
        "out": args.out,
        "index": args.out.join("index.html"),
        "manifest": final_manifest,
        "api": args.out.join("api"),
        "dashboard_schema_version": DASHBOARD_SCHEMA_VERSION,
        "manifest_sha256": manifest_sha256,
        "ledger_record_count": inspection.record_count,
        "ledger_tip_sha256_at_build": inspection.ledger_tip_sha256,
        "runs_total": data.summary["counts"]["runs_total"].clone(),
        "evidence_packets_total": data.summary["counts"]["evidence_packets_total"].clone(),
        "incident_packets_total": data.summary["counts"]["incident_packets_total"].clone(),
        "timeline_files_total": data.timelines.len(),
    }))
}

fn build_dashboard_summary(
    ledger_path: &Path,
    inspection: &LedgerInspection,
    ledger_id: &str,
    generated_at: &str,
    kind_counts: &BTreeMap<String, i64>,
    evidence_hash_verification: &Value,
    evidence_tool_call_ids: &BTreeSet<String>,
) -> BbxResult<Value> {
    let verifier_status_counts = verifier_status_counts(ledger_path, &inspection.records)?;
    let policy_decision_counts = policy_decision_counts(ledger_path, &inspection.records)?;
    let tool_calls_total = kind_counts.get("tool_call").copied().unwrap_or(0);
    let tool_calls_with_evidence = evidence_tool_call_ids.len() as i64;
    let tool_calls_without_evidence = (tool_calls_total - tool_calls_with_evidence).max(0);
    let verifier_total = verifier_status_counts.values().sum::<i64>();
    let passed = verifier_status_counts.get("passed").copied().unwrap_or(0);
    let failed = verifier_status_counts.get("failed").copied().unwrap_or(0);
    let blocked = verifier_status_counts.get("blocked").copied().unwrap_or(0);
    let pass_rate = ratio_value(passed, verifier_total);
    let failure_rate = ratio_value(failed, verifier_total);
    let source_coverage = ratio_value(tool_calls_with_evidence, tool_calls_total);
    let metrics = vec![
        json!({"label": "Runs", "value": kind_counts.get("agent_run").copied().unwrap_or(0)}),
        json!({"label": "Tool calls", "value": tool_calls_total}),
        json!({"label": "Evidence packets", "value": kind_counts.get("evidence_packet").copied().unwrap_or(0)}),
        json!({"label": "Incidents", "value": kind_counts.get("incident_packet").copied().unwrap_or(0)}),
        json!({"label": "Verifier passed", "value": passed}),
        json!({"label": "Verifier failed", "value": failed}),
        json!({"label": "Verifier blocked", "value": blocked}),
        json!({"label": "Unverified actions", "value": tool_calls_without_evidence}),
    ];
    Ok(json!({
        "format": "blackbox_dashboard_summary_json_v1",
        "dashboard_schema_version": DASHBOARD_SCHEMA_VERSION,
        "generated_at": generated_at,
        "source_ledger": {
            "path": ledger_path,
            "source_of_truth": source_of_truth(ledger_path),
            "ledger_id": ledger_id,
            "ledger_schema_version": LEDGER_SCHEMA_VERSION,
            "ledger_record_count": inspection.record_count,
            "ledger_tip_sha256": inspection.ledger_tip_sha256,
            "metadata_record_count": inspection.metadata_record_count,
            "metadata_tip_sha256": inspection.metadata_tip_sha256,
        },
        "counts": {
            "record_kinds": kind_counts,
            "runs_total": kind_counts.get("agent_run").copied().unwrap_or(0),
            "tool_calls_total": tool_calls_total,
            "evidence_packets_total": kind_counts.get("evidence_packet").copied().unwrap_or(0),
            "incident_packets_total": kind_counts.get("incident_packet").copied().unwrap_or(0),
            "verifier_results_total": verifier_total,
            "verifier_statuses": verifier_status_counts,
            "policy_decisions": policy_decision_counts,
        },
        "metrics": metrics,
        "rates": {
            "verifier_pass_rate": pass_rate,
            "verifier_failure_rate": failure_rate,
            "source_of_truth_coverage": source_coverage,
        },
        "source_of_truth_coverage": {
            "tool_calls_total": tool_calls_total,
            "tool_calls_with_evidence": tool_calls_with_evidence,
            "tool_calls_without_evidence": tool_calls_without_evidence,
            "coverage_ratio": source_coverage,
        },
        "evidence_hash_verification": evidence_hash_verification,
    }))
}

fn build_dashboard_runs(
    ledger_path: &Path,
    inspection: &LedgerInspection,
    ledger_id: &str,
    generated_at: &str,
    run_records: &[&LedgerRecordOutput],
    records_by_run: &BTreeMap<String, Vec<&LedgerRecordOutput>>,
) -> BbxResult<Value> {
    let mut seen = BTreeSet::new();
    let mut rows = Vec::new();
    for run_record in run_records {
        let run_id = required_dashboard_payload_str(ledger_path, run_record, "run_id")?;
        if !seen.insert(run_id.to_string()) {
            return Err(dashboard_schema_error(
                ledger_path,
                run_record,
                "payload.run_id",
                format!("duplicate agent_run row for run_id {run_id}"),
            ));
        }
        let records = records_by_run.get(run_id).ok_or_else(|| {
            dashboard_schema_error(
                ledger_path,
                run_record,
                "run_id",
                format!("run_id {run_id} has no records_by_run entry"),
            )
        })?;
        let kinds = ledger_record_kind_counts_from_refs(records);
        let verifier_counts = verifier_status_counts_from_refs(ledger_path, records)?;
        let policy_counts = policy_decision_counts_from_refs(ledger_path, records)?;
        let latest = records
            .iter()
            .max_by_key(|record| record.sequence)
            .expect("run has at least one record");
        let sequence_start = records
            .iter()
            .map(|record| record.sequence)
            .min()
            .expect("run has at least one sequence");
        let sequence_end = records
            .iter()
            .map(|record| record.sequence)
            .max()
            .expect("run has at least one sequence");
        let tool_calls_total = kinds.get("tool_call").copied().unwrap_or(0);
        let evidence_packets_total = kinds.get("evidence_packet").copied().unwrap_or(0);
        let incident_packets_total = kinds.get("incident_packet").copied().unwrap_or(0);
        rows.push(json!({
            "run_id": run_id,
            "tenant_id": run_record.tenant_id,
            "created_at": run_record.created_at,
            "actor": run_record.actor.clone(),
            "task": run_record.payload.get("task").cloned().unwrap_or(Value::Null),
            "runtime": run_record.payload.get("runtime").cloned().unwrap_or(Value::Null),
            "status": dashboard_run_status(&verifier_counts, &policy_counts, incident_packets_total, tool_calls_total, evidence_packets_total),
            "record_count": records.len(),
            "record_kind_counts": kinds,
            "verifier_status_counts": verifier_counts,
            "policy_decision_counts": policy_counts,
            "tool_calls_total": tool_calls_total,
            "evidence_packets_total": evidence_packets_total,
            "incident_packets_total": incident_packets_total,
            "ledger_sequence_range": [sequence_start, sequence_end],
            "latest_record_sha256": latest.hash.get("record_sha256").cloned().unwrap_or(Value::Null),
            "source_of_truth": {
                "ledger": source_of_truth(ledger_path),
                "run_chain_record_sha256": latest.hash.get("run_chain_sha256").cloned().unwrap_or(Value::Null),
            },
            "timeline_api": format!("api/{}", dashboard_timeline_file_name(run_id)),
        }));
    }
    Ok(json!({
        "format": "blackbox_dashboard_runs_json_v1",
        "dashboard_schema_version": DASHBOARD_SCHEMA_VERSION,
        "generated_at": generated_at,
        "source_ledger": dashboard_source_ledger(ledger_path, inspection, ledger_id),
        "runs": rows,
    }))
}

fn build_dashboard_evidence(
    ledger_path: &Path,
    inspection: &LedgerInspection,
    ledger_id: &str,
    generated_at: &str,
    records: &[LedgerRecordOutput],
) -> BbxResult<Value> {
    let mut packets = Vec::new();
    for record in records
        .iter()
        .filter(|record| record.record_kind == "evidence_packet")
    {
        packets.push(json!({
            "evidence_packet_id": required_dashboard_payload_str(ledger_path, record, "evidence_packet_id")?,
            "run_id": required_dashboard_payload_str(ledger_path, record, "run_id")?,
            "tool_call_id": required_dashboard_payload_str(ledger_path, record, "tool_call_id")?,
            "verifier_id": required_dashboard_payload_str(ledger_path, record, "verifier_id")?,
            "verifier_result_id": required_dashboard_payload_str(ledger_path, record, "verifier_result_id")?,
            "verifier_status": required_dashboard_payload_str(ledger_path, record, "verifier_status")?,
            "sequence": record.sequence,
            "record_id": record.record_id,
            "created_at": record.created_at,
            "source_of_truth": record.payload.get("source_of_truth").cloned().unwrap_or(Value::Null),
            "record_refs": record.payload.get("record_refs").cloned().unwrap_or(Value::Null),
            "hashes": record.payload.get("hashes").cloned().unwrap_or(Value::Null),
            "payload_sha256": record.hash.get("payload_sha256").cloned().unwrap_or(Value::Null),
            "record_sha256": record.hash.get("record_sha256").cloned().unwrap_or(Value::Null),
        }));
    }
    Ok(json!({
        "format": "blackbox_dashboard_evidence_json_v1",
        "dashboard_schema_version": DASHBOARD_SCHEMA_VERSION,
        "generated_at": generated_at,
        "source_ledger": dashboard_source_ledger(ledger_path, inspection, ledger_id),
        "evidence_packets": packets,
    }))
}

fn build_dashboard_verifiers(
    ledger_path: &Path,
    inspection: &LedgerInspection,
    ledger_id: &str,
    generated_at: &str,
    records: &[LedgerRecordOutput],
    tool_call_by_id: &BTreeMap<String, &LedgerRecordOutput>,
) -> BbxResult<Value> {
    let mut rows = Vec::new();
    let mut by_tool = BTreeMap::<String, BTreeMap<String, i64>>::new();
    let mut by_connector = BTreeMap::<String, BTreeMap<String, i64>>::new();
    let mut by_agent = BTreeMap::<String, BTreeMap<String, i64>>::new();
    let mut by_tenant = BTreeMap::<String, BTreeMap<String, i64>>::new();
    for record in records
        .iter()
        .filter(|record| record.record_kind == "verifier_result")
    {
        let status = required_dashboard_payload_str(ledger_path, record, "status")?;
        let tool_call_id = required_dashboard_payload_str(ledger_path, record, "tool_call_id")?;
        let tool_name = tool_call_by_id
            .get(tool_call_id)
            .map(|tool| required_dashboard_payload_str(ledger_path, tool, "tool_name"))
            .transpose()?
            .ok_or_else(|| {
                dashboard_schema_error(
                    ledger_path,
                    record,
                    "payload.tool_call_id",
                    format!("verifier_result references missing tool_call_id {tool_call_id}"),
                )
            })?;
        let connector = required_dashboard_payload_str(ledger_path, record, "connector")?;
        let agent_id = required_dashboard_actor_str(ledger_path, record, "agent_id")?;
        increment_nested_count(&mut by_tool, tool_name, status);
        increment_nested_count(&mut by_connector, connector, status);
        increment_nested_count(&mut by_agent, agent_id, status);
        increment_nested_count(&mut by_tenant, &record.tenant_id, status);
        rows.push(json!({
            "verifier_result_id": required_dashboard_payload_str(ledger_path, record, "verifier_result_id")?,
            "run_id": record.run_id,
            "tool_call_id": tool_call_id,
            "tool_name": tool_name,
            "verifier_id": required_dashboard_payload_str(ledger_path, record, "verifier_id")?,
            "connector": connector,
            "status": status,
            "score": record.payload.get("score").cloned().unwrap_or(Value::Null),
            "error_code": record.payload.get("error_code").cloned().unwrap_or(Value::Null),
            "error_message": record.payload.get("error_message").cloned().unwrap_or(Value::Null),
            "field_path": record.payload.get("field_path").cloned().unwrap_or(Value::Null),
            "remediation": record.payload.get("remediation").cloned().unwrap_or(Value::Null),
            "source_of_truth": record.payload.get("source_of_truth").cloned().unwrap_or(Value::Null),
            "sequence": record.sequence,
            "record_sha256": record.hash.get("record_sha256").cloned().unwrap_or(Value::Null),
        }));
    }
    Ok(json!({
        "format": "blackbox_dashboard_verifier_status_json_v1",
        "dashboard_schema_version": DASHBOARD_SCHEMA_VERSION,
        "generated_at": generated_at,
        "source_ledger": dashboard_source_ledger(ledger_path, inspection, ledger_id),
        "counts": {
            "by_status": verifier_status_counts(ledger_path, records)?,
            "by_tool": by_tool,
            "by_connector": by_connector,
            "by_agent": by_agent,
            "by_tenant": by_tenant,
        },
        "verifier_results": rows,
    }))
}

fn build_dashboard_errors(
    ledger_path: &Path,
    inspection: &LedgerInspection,
    ledger_id: &str,
    generated_at: &str,
    records: &[LedgerRecordOutput],
    tool_call_by_id: &BTreeMap<String, &LedgerRecordOutput>,
) -> BbxResult<Value> {
    let mut groups = BTreeMap::<String, DashboardErrorGroup>::new();
    for record in records.iter().filter(|record| {
        record.record_kind == "verifier_result"
            || record.record_kind == "incident_packet"
            || record.record_kind == "policy_decision"
    }) {
        let error_code = record.payload.get("error_code").and_then(Value::as_str);
        let include_policy_denial = record.record_kind == "policy_decision"
            && record.payload.get("status").and_then(Value::as_str) == Some("denied");
        let Some(code) = error_code.filter(|code| !code.trim().is_empty()) else {
            if include_policy_denial {
                return Err(dashboard_schema_error(
                    ledger_path,
                    record,
                    "payload.error_code",
                    "denied policy_decision is missing error_code",
                ));
            }
            continue;
        };
        let group = groups
            .entry(code.to_string())
            .or_insert_with(|| DashboardErrorGroup::new(code));
        group.count += 1;
        group.record_kinds.insert(record.record_kind.clone());
        group.first_sequence = group.first_sequence.min(record.sequence);
        group.last_sequence = group.last_sequence.max(record.sequence);
        if let Some(remediation) = record.payload.get("remediation").and_then(Value::as_str) {
            group.remediations.insert(remediation.to_string());
        }
        if let Some(source) = record.payload.get("source_of_truth") {
            group.source_examples.push(source.clone());
        }
        if let Some(tool_call_id) = record.payload.get("tool_call_id").and_then(Value::as_str) {
            if let Some(tool_record) = tool_call_by_id.get(tool_call_id) {
                group.tools.insert(
                    required_dashboard_payload_str(ledger_path, tool_record, "tool_name")?
                        .to_string(),
                );
            }
        }
    }
    let error_codes = groups
        .into_values()
        .map(|group| group.to_json())
        .collect::<Vec<_>>();
    Ok(json!({
        "format": "blackbox_dashboard_errors_json_v1",
        "dashboard_schema_version": DASHBOARD_SCHEMA_VERSION,
        "generated_at": generated_at,
        "source_ledger": dashboard_source_ledger(ledger_path, inspection, ledger_id),
        "error_codes": error_codes,
    }))
}

fn build_dashboard_exports(
    ledger_path: &Path,
    inspection: &LedgerInspection,
    ledger_id: &str,
    generated_at: &str,
    run_records: &[&LedgerRecordOutput],
) -> BbxResult<Value> {
    let mut run_exports = Vec::new();
    for run_record in run_records {
        let run_id = required_dashboard_payload_str(ledger_path, run_record, "run_id")?;
        run_exports.push(json!({
            "run_id": run_id,
            "evidence_export_command": [
                "blackbox", "export", "evidence",
                "--ledger", ledger_path.display().to_string(),
                "--run-id", run_id,
                "--out", format!("evidence_{}.json", safe_filename_fragment(run_id)),
            ],
            "timeline_api": format!("api/{}", dashboard_timeline_file_name(run_id)),
        }));
    }
    Ok(json!({
        "format": "blackbox_dashboard_exports_json_v1",
        "dashboard_schema_version": DASHBOARD_SCHEMA_VERSION,
        "generated_at": generated_at,
        "source_ledger": dashboard_source_ledger(ledger_path, inspection, ledger_id),
        "exports": {
            "evidence_by_run": run_exports,
            "episodes_export_command": [
                "blackbox", "export", "episodes",
                "--ledger", ledger_path.display().to_string(),
                "--out", "episodes-export"
            ],
            "dashboard_files": [
                "index.html",
                "manifest.json",
                "api/summary.json",
                "api/runs.json",
                "api/evidence.json",
                "api/verifier-status.json",
                "api/errors.json",
                "api/exports.json"
            ],
        },
    }))
}

fn build_dashboard_timelines(
    ledger_path: &Path,
    inspection: &LedgerInspection,
    run_records: &[&LedgerRecordOutput],
) -> BbxResult<Vec<(String, String, Value)>> {
    let mut timelines = Vec::new();
    for run_record in run_records {
        let run_id = required_dashboard_payload_str(ledger_path, run_record, "run_id")?;
        let report = build_run_replay_report(ledger_path, inspection, run_id)?;
        timelines.push((
            run_id.to_string(),
            dashboard_timeline_file_name(run_id),
            json!({
                "format": "blackbox_dashboard_timeline_json_v1",
                "dashboard_schema_version": DASHBOARD_SCHEMA_VERSION,
                "source_of_truth": source_of_truth(ledger_path),
                "run_id": run_id,
                "report": report,
            }),
        ));
    }
    Ok(timelines)
}

fn write_episode_export_directory(
    args: &ExportEpisodesArgs,
    inspection: &LedgerInspection,
    ledger_id: &str,
    rows: &[Value],
    temp_dir: &Path,
) -> BbxResult<Value> {
    fs::create_dir(temp_dir)
        .map_err(|err| BlackboxError::io(temp_dir, "create episode export temp directory", err))?;
    let episodes_path = temp_dir.join("episodes.jsonl");
    let manifest_path = temp_dir.join("manifest.json");
    let evals_dir = temp_dir.join("evals");
    fs::create_dir(&evals_dir)
        .map_err(|err| BlackboxError::io(&evals_dir, "create episode eval directory", err))?;

    write_episode_jsonl(&episodes_path, rows)?;
    let tmp_readback = readback_episode_jsonl(&episodes_path)?;
    if tmp_readback.line_count != rows.len() {
        return Err(BlackboxError::evidence_integrity_for_connector(
            "EPISODE_JSONL_READBACK_MISMATCH",
            FS_CONNECTOR_ID,
            file_source_of_truth(&episodes_path),
            "episodes.jsonl",
            format!(
                "episodes temp readback count mismatch: expected {}, got {}",
                rows.len(),
                tmp_readback.line_count
            ),
            "delete the temp export and rerun after inspecting filesystem/storage consistency",
        ));
    }

    let eval_fixtures = write_eval_fixtures(&evals_dir, rows)?;
    let sequence_range = exported_sequence_range(rows)?;
    let manifest = build_episodes_manifest(
        args,
        inspection,
        ledger_id,
        &episodes_path,
        &tmp_readback,
        &eval_fixtures,
        sequence_range,
    );
    write_json_file_new(&manifest_path, &manifest, "episode export manifest")?;
    let manifest_readback = read_json_file(&manifest_path, "episode export manifest readback")?;
    if manifest_readback["status"] != "ok"
        || manifest_readback["output"]["episodes_sha256"] != tmp_readback.sha256
    {
        return Err(BlackboxError::evidence_integrity_for_connector(
            "EPISODE_MANIFEST_READBACK_MISMATCH",
            FS_CONNECTOR_ID,
            file_source_of_truth(&manifest_path),
            "manifest.json",
            "episode manifest readback does not match the JSONL hash written in the manifest",
            "delete the corrupt export and rerun after inspecting filesystem/storage consistency",
        ));
    }

    promote_episode_export_dir(temp_dir, &args.out, args.force)?;

    let final_episodes = args.out.join("episodes.jsonl");
    let final_manifest = args.out.join("manifest.json");
    let final_readback = readback_episode_jsonl(&final_episodes)?;
    if final_readback.sha256 != tmp_readback.sha256
        || final_readback.line_count != tmp_readback.line_count
    {
        return Err(BlackboxError::evidence_integrity_for_connector(
            "EPISODE_EXPORT_READBACK_MISMATCH",
            FS_CONNECTOR_ID,
            file_source_of_truth(&final_episodes),
            "episodes.jsonl",
            "final episodes.jsonl readback changed after directory promotion",
            "treat the export target as unreliable; inspect filesystem/storage before retrying",
        ));
    }
    let final_manifest_readback = read_json_file(&final_manifest, "final episode manifest")?;
    if final_manifest_readback["output"]["episodes_sha256"] != final_readback.sha256 {
        return Err(BlackboxError::evidence_integrity_for_connector(
            "EPISODE_MANIFEST_READBACK_MISMATCH",
            FS_CONNECTOR_ID,
            file_source_of_truth(&final_manifest),
            "manifest.json.output.episodes_sha256",
            "final manifest hash does not match final episodes.jsonl readback",
            "delete the corrupt export and rerun after inspecting filesystem/storage consistency",
        ));
    }

    Ok(json!({
        "status": "ok",
        "command": "blackbox export episodes",
        "source_of_truth": file_source_of_truth(&final_episodes),
        "ledger_source_of_truth": source_of_truth(&args.ledger),
        "out": args.out,
        "episodes": final_episodes,
        "manifest": final_manifest,
        "evals": args.out.join("evals"),
        "episodes_total": final_readback.line_count,
        "episodes_sha256": final_readback.sha256,
        "episodes_size_bytes": final_readback.size_bytes,
        "counts": {
            "rows_by_kind": final_readback.rows_by_kind,
            "splits": final_readback.splits,
        },
        "eval_fixture_count": final_manifest_readback["evals"]["fixture_count"].clone(),
        "ledger_sequence_range": final_manifest_readback["source_ledger"]["ledger_sequence_range"].clone(),
        "ledger_tip_sha256_at_export": inspection.ledger_tip_sha256,
    }))
}

fn validate_export_output_path(path: &Path) -> BbxResult<()> {
    if path.as_os_str().is_empty() {
        return Err(BlackboxError::new_with_connector(
            ErrorKind::InvalidInput,
            "BBX_EXPORT_PATH_INVALID",
            "--out must not be empty",
            None,
            FS_CONNECTOR_ID,
            Some("--out".to_string()),
            "provide a non-empty export output path",
        ));
    }
    if path.exists() && path.is_dir() {
        return Err(BlackboxError::new_with_connector(
            ErrorKind::InvalidInput,
            "BBX_EXPORT_PATH_INVALID",
            "--out points to a directory, expected a JSON file path",
            Some(file_source_of_truth(path)),
            FS_CONNECTOR_ID,
            Some("--out".to_string()),
            "provide a file path, not a directory",
        ));
    }
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() && (!parent.exists() || !parent.is_dir()) {
            return Err(BlackboxError::new_with_connector(
                ErrorKind::InvalidInput,
                "BBX_EXPORT_PATH_INVALID",
                format!(
                    "export parent directory does not exist: {}",
                    parent.display()
                ),
                Some(file_source_of_truth(path)),
                FS_CONNECTOR_ID,
                Some("--out".to_string()),
                "create the parent directory explicitly and rerun the export",
            ));
        }
    }
    Ok(())
}

fn validate_export_directory_path(path: &Path, force: bool) -> BbxResult<()> {
    if path.as_os_str().is_empty() {
        return Err(BlackboxError::new_with_connector(
            ErrorKind::InvalidInput,
            "BBX_EXPORT_PATH_INVALID",
            "--out must not be empty",
            None,
            FS_CONNECTOR_ID,
            Some("--out".to_string()),
            "provide a non-empty export output directory",
        ));
    }
    if path.exists() {
        if !path.is_dir() {
            return Err(BlackboxError::new_with_connector(
                ErrorKind::InvalidInput,
                "BBX_EXPORT_PATH_INVALID",
                "--out exists and is not a directory",
                Some(file_source_of_truth(path)),
                FS_CONNECTOR_ID,
                Some("--out".to_string()),
                "provide a directory path or remove the existing file",
            ));
        }
        if !force {
            return Err(BlackboxError::new_with_connector(
                ErrorKind::InvalidInput,
                "BBX_OUTPUT_EXISTS",
                format!(
                    "episode export output directory already exists: {}",
                    path.display()
                ),
                Some(file_source_of_truth(path)),
                FS_CONNECTOR_ID,
                Some("--out".to_string()),
                "rerun with --force only when replacing this export directory is intentional",
            ));
        }
    }
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() && (!parent.exists() || !parent.is_dir()) {
            return Err(BlackboxError::new_with_connector(
                ErrorKind::InvalidInput,
                "BBX_EXPORT_PATH_INVALID",
                format!(
                    "episode export parent directory does not exist: {}",
                    parent.display()
                ),
                Some(file_source_of_truth(path)),
                FS_CONNECTOR_ID,
                Some("--out".to_string()),
                "create the parent directory explicitly and rerun the export",
            ));
        }
    }
    Ok(())
}

fn validate_tool_calls_have_evidence(
    ledger_path: &Path,
    records: &[LedgerRecordOutput],
) -> BbxResult<()> {
    let mut evidenced_tool_calls = BTreeSet::new();
    for record in records
        .iter()
        .filter(|record| record.record_kind == "evidence_packet")
    {
        if let Some(tool_call_id) = record.payload.get("tool_call_id").and_then(Value::as_str) {
            evidenced_tool_calls.insert(tool_call_id.to_string());
        }
    }

    for record in records
        .iter()
        .filter(|record| record.record_kind == "tool_call")
    {
        let tool_call_id = record
            .payload
            .get("tool_call_id")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                BlackboxError::evidence_integrity(
                    "EPISODE_SOURCE_SCHEMA_INVALID",
                    source_of_truth(ledger_path),
                    format!("ledger_records[{}].payload.tool_call_id", record.sequence),
                    "tool_call payload is missing tool_call_id",
                    "treat the ledger as inconsistent and inspect the append-only chain",
                )
            })?;
        if !evidenced_tool_calls.contains(tool_call_id) {
            return Err(BlackboxError::new(
                ErrorKind::InvalidInput,
                "BBX_VERIFIER_MISSING_FOR_MUTATING_ACTION",
                format!("tool_call_id {tool_call_id} has no evidence_packet verifier output"),
                Some(source_of_truth(ledger_path)),
                Some(format!("ledger_records[{}].payload.tool_call_id", record.sequence)),
                "run blackbox verify for every mutating tool_call before exporting training episodes",
            ));
        }
    }
    Ok(())
}

fn build_episode_rows(ledger_path: &Path, inspection: &LedgerInspection) -> BbxResult<Vec<Value>> {
    let records_by_sequence = inspection
        .records
        .iter()
        .map(|record| (record.sequence, record))
        .collect::<BTreeMap<_, _>>();
    let mut incidents_by_evidence_id = BTreeMap::<String, Vec<&LedgerRecordOutput>>::new();
    for record in inspection
        .records
        .iter()
        .filter(|record| record.record_kind == "incident_packet")
    {
        if let Some(evidence_id) = record
            .payload
            .get("evidence_packet_id")
            .and_then(Value::as_str)
        {
            incidents_by_evidence_id
                .entry(evidence_id.to_string())
                .or_default()
                .push(record);
        }
    }

    let mut rows = Vec::new();
    for evidence_record in inspection
        .records
        .iter()
        .filter(|record| record.record_kind == "evidence_packet")
    {
        let evidence_id = required_payload_str(ledger_path, evidence_record, "evidence_packet_id")?;
        let run_record = required_evidence_ref_record(
            ledger_path,
            evidence_record,
            &records_by_sequence,
            "run",
            "agent_run",
        )?;
        let tool_record = required_evidence_ref_record(
            ledger_path,
            evidence_record,
            &records_by_sequence,
            "tool_call",
            "tool_call",
        )?;
        let expected_record = required_evidence_ref_record(
            ledger_path,
            evidence_record,
            &records_by_sequence,
            "expected_delta",
            "expected_delta",
        )?;
        let verifier_record = required_evidence_ref_record(
            ledger_path,
            evidence_record,
            &records_by_sequence,
            "verifier_result",
            "verifier_result",
        )?;
        let readback_record = optional_evidence_ref_record(
            ledger_path,
            evidence_record,
            &records_by_sequence,
            "readback_snapshot",
            "readback_snapshot",
        )?;
        let incidents = incidents_by_evidence_id
            .get(evidence_id)
            .cloned()
            .unwrap_or_default();

        let row = build_verified_episode_row(
            ledger_path,
            inspection,
            evidence_record,
            run_record,
            tool_record,
            expected_record,
            readback_record,
            verifier_record,
            incidents.first().copied(),
        )?;
        rows.push(row);

        for incident in incidents {
            rows.push(build_incident_episode_row(
                ledger_path,
                inspection,
                evidence_record,
                run_record,
                tool_record,
                expected_record,
                readback_record,
                verifier_record,
                incident,
            )?);
        }
    }
    Ok(rows)
}

#[allow(clippy::too_many_arguments)]
fn build_verified_episode_row(
    ledger_path: &Path,
    inspection: &LedgerInspection,
    evidence_record: &LedgerRecordOutput,
    run_record: &LedgerRecordOutput,
    tool_record: &LedgerRecordOutput,
    expected_record: &LedgerRecordOutput,
    readback_record: Option<&LedgerRecordOutput>,
    verifier_record: &LedgerRecordOutput,
    incident_record: Option<&LedgerRecordOutput>,
) -> BbxResult<Value> {
    let evidence_id = required_payload_str(ledger_path, evidence_record, "evidence_packet_id")?;
    let run_id = required_payload_str(ledger_path, evidence_record, "run_id")?;
    let tool_call_id = required_payload_str(ledger_path, evidence_record, "tool_call_id")?;
    let verifier_status = required_payload_str(ledger_path, verifier_record, "status")?;
    let split = match verifier_status {
        "passed" | "failed" | "blocked" => verifier_status,
        other => {
            return Err(BlackboxError::evidence_integrity(
                "EPISODE_SOURCE_SCHEMA_INVALID",
                source_of_truth(ledger_path),
                format!(
                    "ledger_records[{}].payload.status",
                    verifier_record.sequence
                ),
                format!("verifier_result status {other:?} is not exportable"),
                "fix the verifier status so it is passed, failed, or blocked before exporting",
            ));
        }
    };
    let sequence_range = row_sequence_range(&[
        Some(run_record),
        Some(tool_record),
        Some(expected_record),
        readback_record,
        Some(verifier_record),
        Some(evidence_record),
        incident_record,
    ])?;
    let source = episode_source(
        ledger_path,
        inspection,
        run_record,
        tool_record,
        evidence_record,
        incident_record,
        sequence_range,
    );
    Ok(json!({
        "schema_version": LEDGER_SCHEMA_VERSION,
        "episode_schema_version": EPISODE_EXPORT_SCHEMA_VERSION,
        "record_kind": "verified_agent_episode",
        "dataset_split": split,
        "episode_id": format!("{run_id}:evidence:{evidence_id}"),
        "source": source,
        "state": {
            "task": run_record.payload.get("task").cloned().unwrap_or(Value::Null),
            "runtime": run_record.payload.get("runtime").cloned().unwrap_or(Value::Null),
            "policy_context": tool_record.payload.get("authorization").cloned().unwrap_or(Value::Null),
            "available_tools": [tool_record.payload.get("tool_name").cloned().unwrap_or(Value::Null)],
            "source_of_truth": evidence_record.payload.get("source_of_truth").cloned().unwrap_or(Value::Null),
        },
        "action": {
            "tool_call_id": tool_call_id,
            "tool_name": tool_record.payload.get("tool_name").cloned().unwrap_or(Value::Null),
            "capability_id": tool_record.payload.get("capability_id").cloned().unwrap_or(Value::Null),
            "tool_input_redacted": tool_record.payload.get("input_redacted").cloned().unwrap_or(Value::Null),
            "execution": tool_record.payload.get("execution").cloned().unwrap_or(Value::Null),
            "model_decision_redacted": Value::Null,
        },
        "expected_persisted_delta": expected_record.payload.get("expected_persisted_delta").cloned().unwrap_or(Value::Null),
        "actual_readback": {
            "readback_snapshot": readback_record.map(|record| record.payload.clone()).unwrap_or(Value::Null),
            "verifier_actual_readback": verifier_record.payload.get("actual_readback").cloned().unwrap_or(Value::Null),
        },
        "verifier_status": {
            "status": verifier_status,
            "verifier_id": verifier_record.payload.get("verifier_id").cloned().unwrap_or(Value::Null),
            "verifier_result_id": verifier_record.payload.get("verifier_result_id").cloned().unwrap_or(Value::Null),
            "score": verifier_record.payload.get("score").cloned().unwrap_or(Value::Null),
        },
        "error_code": verifier_record.payload.get("error_code").cloned().unwrap_or(Value::Null),
        "remediation": verifier_record.payload.get("remediation").cloned().unwrap_or(Value::Null),
        "incident": incident_record.map(|record| record.payload.clone()).unwrap_or(Value::Null),
        "provenance": evidence_record.payload.get("provenance").cloned().unwrap_or(Value::Null),
    }))
}

#[allow(clippy::too_many_arguments)]
fn build_incident_episode_row(
    ledger_path: &Path,
    inspection: &LedgerInspection,
    evidence_record: &LedgerRecordOutput,
    run_record: &LedgerRecordOutput,
    tool_record: &LedgerRecordOutput,
    expected_record: &LedgerRecordOutput,
    readback_record: Option<&LedgerRecordOutput>,
    verifier_record: &LedgerRecordOutput,
    incident_record: &LedgerRecordOutput,
) -> BbxResult<Value> {
    let evidence_id = required_payload_str(ledger_path, evidence_record, "evidence_packet_id")?;
    let incident_id = required_payload_str(ledger_path, incident_record, "incident_packet_id")?;
    let run_id = required_payload_str(ledger_path, evidence_record, "run_id")?;
    let sequence_range = row_sequence_range(&[
        Some(run_record),
        Some(tool_record),
        Some(expected_record),
        readback_record,
        Some(verifier_record),
        Some(evidence_record),
        Some(incident_record),
    ])?;
    let source = episode_source(
        ledger_path,
        inspection,
        run_record,
        tool_record,
        evidence_record,
        Some(incident_record),
        sequence_range,
    );
    Ok(json!({
        "schema_version": LEDGER_SCHEMA_VERSION,
        "episode_schema_version": EPISODE_EXPORT_SCHEMA_VERSION,
        "record_kind": "incident_agent_episode",
        "dataset_split": "incident",
        "episode_id": format!("{run_id}:incident:{incident_id}"),
        "source": source,
        "state": {
            "task": run_record.payload.get("task").cloned().unwrap_or(Value::Null),
            "incident": incident_record.payload.clone(),
            "source_of_truth": incident_record.payload.get("source_of_truth").cloned().unwrap_or(Value::Null),
        },
        "action": {
            "tool_name": tool_record.payload.get("tool_name").cloned().unwrap_or(Value::Null),
            "capability_id": tool_record.payload.get("capability_id").cloned().unwrap_or(Value::Null),
            "tool_input_redacted": tool_record.payload.get("input_redacted").cloned().unwrap_or(Value::Null),
            "execution": tool_record.payload.get("execution").cloned().unwrap_or(Value::Null),
            "trigger": incident_record.payload.get("trigger").cloned().unwrap_or(Value::Null),
        },
        "expected_persisted_delta": expected_record.payload.get("expected_persisted_delta").cloned().unwrap_or(Value::Null),
        "actual_readback": {
            "readback_snapshot": readback_record.map(|record| record.payload.clone()).unwrap_or(Value::Null),
            "verifier_result": verifier_record.payload.clone(),
            "incident_packet": incident_record.payload.clone(),
        },
        "verifier_status": {
            "status": "incident",
            "linked_verifier_status": verifier_record.payload.get("status").cloned().unwrap_or(Value::Null),
            "verifier_id": verifier_record.payload.get("verifier_id").cloned().unwrap_or(Value::Null),
        },
        "error_code": incident_record.payload.get("error_code").cloned().unwrap_or(Value::Null),
        "remediation": incident_record.payload.get("remediation").cloned().unwrap_or(Value::Null),
        "provenance": evidence_record.payload.get("provenance").cloned().unwrap_or(Value::Null),
        "evidence_packet_id": evidence_id,
    }))
}

fn episode_source(
    ledger_path: &Path,
    inspection: &LedgerInspection,
    run_record: &LedgerRecordOutput,
    tool_record: &LedgerRecordOutput,
    evidence_record: &LedgerRecordOutput,
    incident_record: Option<&LedgerRecordOutput>,
    sequence_range: (i64, i64),
) -> Value {
    json!({
        "tenant_id": evidence_record.tenant_id,
        "run_id": evidence_record.run_id,
        "step_id": evidence_record.step_id,
        "tool_call_id": evidence_record.payload.get("tool_call_id").cloned().unwrap_or(Value::Null),
        "evidence_packet_id": evidence_record.payload.get("evidence_packet_id").cloned().unwrap_or(Value::Null),
        "evidence_packet_sequence": evidence_record.sequence,
        "evidence_packet_record_sha256": evidence_record.hash.get("record_sha256").cloned().unwrap_or(Value::Null),
        "incident_packet_id": incident_record.and_then(|record| record.payload.get("incident_packet_id")).cloned().unwrap_or(Value::Null),
        "incident_packet_sequence": incident_record.map(|record| json!(record.sequence)).unwrap_or(Value::Null),
        "ledger_sequence_range": [sequence_range.0, sequence_range.1],
        "ledger_tip_sha256_at_export": inspection.ledger_tip_sha256,
        "ledger_source_of_truth": source_of_truth(ledger_path),
        "run_record": ledger_record_ref_from_record("run", run_record),
        "tool_call_record": ledger_record_ref_from_record("tool_call", tool_record),
    })
}

fn ledger_record_ref_from_record(role: &str, record: &LedgerRecordOutput) -> Value {
    json!({
        "role": role,
        "sequence": record.sequence,
        "record_id": record.record_id,
        "record_kind": record.record_kind,
        "payload_sha256": record.hash.get("payload_sha256").cloned().unwrap_or(Value::Null),
        "record_sha256": record.hash.get("record_sha256").cloned().unwrap_or(Value::Null),
    })
}

fn required_payload_str<'a>(
    ledger_path: &Path,
    record: &'a LedgerRecordOutput,
    field: &'static str,
) -> BbxResult<&'a str> {
    record
        .payload
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            BlackboxError::evidence_integrity(
                "EPISODE_SOURCE_SCHEMA_INVALID",
                source_of_truth(ledger_path),
                format!("ledger_records[{}].payload.{field}", record.sequence),
                format!(
                    "{} payload is missing non-empty {field}",
                    record.record_kind
                ),
                "treat the source ledger as incomplete and rerun verification before export",
            )
        })
}

fn required_evidence_ref_record<'a>(
    ledger_path: &Path,
    evidence_record: &LedgerRecordOutput,
    records_by_sequence: &BTreeMap<i64, &'a LedgerRecordOutput>,
    role: &'static str,
    expected_kind: &'static str,
) -> BbxResult<&'a LedgerRecordOutput> {
    optional_evidence_ref_record(
        ledger_path,
        evidence_record,
        records_by_sequence,
        role,
        expected_kind,
    )?
    .ok_or_else(|| {
        BlackboxError::evidence_integrity(
            "EPISODE_SOURCE_SCHEMA_INVALID",
            source_of_truth(ledger_path),
            format!(
                "ledger_records[{}].payload.record_refs",
                evidence_record.sequence
            ),
            format!("evidence_packet is missing required {role} reference"),
            "rebuild the evidence packet from immutable ledger records before export",
        )
    })
}

fn optional_evidence_ref_record<'a>(
    ledger_path: &Path,
    evidence_record: &LedgerRecordOutput,
    records_by_sequence: &BTreeMap<i64, &'a LedgerRecordOutput>,
    role: &'static str,
    expected_kind: &'static str,
) -> BbxResult<Option<&'a LedgerRecordOutput>> {
    let Some(sequence) = evidence_ref_sequence(ledger_path, evidence_record, role)? else {
        return Ok(None);
    };
    let record = records_by_sequence.get(&sequence).copied().ok_or_else(|| {
        BlackboxError::evidence_integrity(
            "EVIDENCE_HASH_MISMATCH",
            source_of_truth(ledger_path),
            format!(
                "ledger_records[{}].payload.record_refs.{role}",
                evidence_record.sequence
            ),
            format!("evidence_packet references missing ledger sequence {sequence}"),
            "treat the evidence packet as invalid and rebuild from an intact ledger",
        )
    })?;
    if record.record_kind != expected_kind {
        return Err(BlackboxError::evidence_integrity(
            "EPISODE_SOURCE_SCHEMA_INVALID",
            source_of_truth(ledger_path),
            format!(
                "ledger_records[{}].payload.record_refs.{role}",
                evidence_record.sequence
            ),
            format!(
                "evidence role {role} points to record_kind {}, expected {expected_kind}",
                record.record_kind
            ),
            "rebuild the evidence packet with correct role references before export",
        ));
    }
    Ok(Some(record))
}

fn evidence_ref_sequence(
    ledger_path: &Path,
    record: &LedgerRecordOutput,
    role: &str,
) -> BbxResult<Option<i64>> {
    let refs = record
        .payload
        .get("record_refs")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            BlackboxError::evidence_integrity(
                "EPISODE_SOURCE_SCHEMA_INVALID",
                source_of_truth(ledger_path),
                format!("ledger_records[{}].payload.record_refs", record.sequence),
                "evidence_packet payload is missing record_refs array",
                "rebuild the evidence packet from immutable ledger records before export",
            )
        })?;
    for item in refs {
        if item.get("role").and_then(Value::as_str) == Some(role) {
            let sequence = item
                .get("sequence")
                .and_then(Value::as_i64)
                .ok_or_else(|| {
                    BlackboxError::evidence_integrity(
                        "EPISODE_SOURCE_SCHEMA_INVALID",
                        source_of_truth(ledger_path),
                        format!(
                            "ledger_records[{}].payload.record_refs.{role}.sequence",
                            record.sequence
                        ),
                        "evidence record reference is missing sequence",
                        "rebuild the evidence packet from immutable ledger records before export",
                    )
                })?;
            return Ok(Some(sequence));
        }
    }
    Ok(None)
}

fn row_sequence_range(records: &[Option<&LedgerRecordOutput>]) -> BbxResult<(i64, i64)> {
    let sequences = records
        .iter()
        .filter_map(|record| record.map(|record| record.sequence))
        .collect::<Vec<_>>();
    let Some(min) = sequences.iter().min().copied() else {
        return Err(BlackboxError::invalid(
            "episode row has no source ledger records",
            "report this as a bug; episode rows must be built from physical ledger rows",
        ));
    };
    let max = sequences
        .iter()
        .max()
        .copied()
        .expect("non-empty sequence list has max");
    Ok((min, max))
}

fn exported_sequence_range(rows: &[Value]) -> BbxResult<(i64, i64)> {
    let mut min_sequence: Option<i64> = None;
    let mut max_sequence: Option<i64> = None;
    for row in rows {
        let range = row
            .pointer("/source/ledger_sequence_range")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                BlackboxError::invalid(
                    "episode row is missing source.ledger_sequence_range",
                    "report this as a bug; episode rows must include a physical ledger sequence range",
                )
            })?;
        if range.len() != 2 {
            return Err(BlackboxError::invalid(
                "episode row source.ledger_sequence_range must contain two integers",
                "report this as a bug; episode rows must include [first_sequence,last_sequence]",
            ));
        }
        let start = range[0].as_i64().ok_or_else(|| {
            BlackboxError::invalid(
                "episode row source.ledger_sequence_range[0] must be integer",
                "report this as a bug; episode rows must include integer sequence bounds",
            )
        })?;
        let end = range[1].as_i64().ok_or_else(|| {
            BlackboxError::invalid(
                "episode row source.ledger_sequence_range[1] must be integer",
                "report this as a bug; episode rows must include integer sequence bounds",
            )
        })?;
        min_sequence = Some(min_sequence.map_or(start, |current| current.min(start)));
        max_sequence = Some(max_sequence.map_or(end, |current| current.max(end)));
    }
    match (min_sequence, max_sequence) {
        (Some(start), Some(end)) => Ok((start, end)),
        _ => Err(BlackboxError::invalid(
            "episode export has no rows",
            "run verifiers before exporting episodes",
        )),
    }
}

#[derive(Debug, Clone, Serialize)]
struct EpisodeJsonlReadback {
    line_count: usize,
    rows_by_kind: BTreeMap<String, i64>,
    splits: BTreeMap<String, i64>,
    first_episode_id: Option<String>,
    last_episode_id: Option<String>,
    sha256: String,
    size_bytes: u64,
}

fn write_episode_jsonl(path: &Path, rows: &[Value]) -> BbxResult<()> {
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|err| BlackboxError::io(path, "create episode JSONL", err))?;
    let mut writer = BufWriter::new(file);
    for row in rows {
        serde_json::to_writer(&mut writer, row).map_err(|err| {
            BlackboxError::new_with_connector(
                ErrorKind::InvalidInput,
                "EPISODE_JSONL_SERIALIZE_FAILED",
                format!("failed to serialize episode row: {err}"),
                Some(file_source_of_truth(path)),
                FS_CONNECTOR_ID,
                Some("episodes.jsonl".to_string()),
                "inspect the episode row JSON shape and rerun export",
            )
        })?;
        writer
            .write_all(b"\n")
            .map_err(|err| BlackboxError::io(path, "write episode JSONL newline", err))?;
    }
    writer
        .flush()
        .map_err(|err| BlackboxError::io(path, "flush episode JSONL", err))?;
    Ok(())
}

fn readback_episode_jsonl(path: &Path) -> BbxResult<EpisodeJsonlReadback> {
    let sha256 = sha256_file(path)?;
    let size_bytes = fs::metadata(path)
        .map_err(|err| BlackboxError::io(path, "stat episode JSONL", err))?
        .len();
    let file =
        File::open(path).map_err(|err| BlackboxError::io(path, "open episode JSONL", err))?;
    let reader = BufReader::new(file);
    let mut line_count = 0usize;
    let mut rows_by_kind = BTreeMap::<String, i64>::new();
    let mut splits = BTreeMap::<String, i64>::new();
    for split in ["passed", "failed", "blocked", "incident", "edge_case"] {
        splits.insert(split.to_string(), 0);
    }
    let mut first_episode_id = None;
    let mut last_episode_id = None;
    for (idx, line_result) in reader.lines().enumerate() {
        let line =
            line_result.map_err(|err| BlackboxError::io(path, "read episode JSONL line", err))?;
        if line.trim().is_empty() {
            return Err(BlackboxError::evidence_integrity_for_connector(
                "EPISODE_JSONL_INVALID",
                FS_CONNECTOR_ID,
                file_source_of_truth(path),
                format!("episodes.jsonl:{}", idx + 1),
                "blank JSONL lines are invalid episode records",
                "regenerate the export; every line must be one complete JSON object",
            ));
        }
        let value: Value = serde_json::from_str(&line).map_err(|err| {
            BlackboxError::evidence_integrity_for_connector(
                "EPISODE_JSONL_INVALID",
                FS_CONNECTOR_ID,
                file_source_of_truth(path),
                format!("episodes.jsonl:{}", idx + 1),
                format!("episode JSONL line does not parse as JSON: {err}"),
                "regenerate the export; every line must be one complete JSON object",
            )
        })?;
        validate_episode_row(path, idx + 1, &value)?;
        let episode_id = value["episode_id"]
            .as_str()
            .expect("validate_episode_row checked episode_id")
            .to_string();
        if first_episode_id.is_none() {
            first_episode_id = Some(episode_id.clone());
        }
        last_episode_id = Some(episode_id);
        let kind = value["record_kind"]
            .as_str()
            .expect("validate_episode_row checked record_kind")
            .to_string();
        let split = value["dataset_split"]
            .as_str()
            .expect("validate_episode_row checked dataset_split")
            .to_string();
        *rows_by_kind.entry(kind).or_insert(0) += 1;
        *splits.entry(split).or_insert(0) += 1;
        line_count += 1;
    }
    Ok(EpisodeJsonlReadback {
        line_count,
        rows_by_kind,
        splits,
        first_episode_id,
        last_episode_id,
        sha256,
        size_bytes,
    })
}

fn validate_episode_row(path: &Path, line: usize, value: &Value) -> BbxResult<()> {
    if !value.is_object() {
        return Err(episode_jsonl_invalid(
            path,
            line,
            "$",
            "episode row must be a JSON object",
        ));
    }
    if value.get("schema_version").and_then(Value::as_i64) != Some(LEDGER_SCHEMA_VERSION) {
        return Err(episode_jsonl_invalid(
            path,
            line,
            "schema_version",
            "episode row schema_version does not match current ledger schema",
        ));
    }
    if value.get("episode_schema_version").and_then(Value::as_i64)
        != Some(EPISODE_EXPORT_SCHEMA_VERSION)
    {
        return Err(episode_jsonl_invalid(
            path,
            line,
            "episode_schema_version",
            "episode row has unsupported episode schema version",
        ));
    }
    for key in [
        "record_kind",
        "dataset_split",
        "episode_id",
        "source",
        "state",
        "action",
        "expected_persisted_delta",
        "actual_readback",
        "verifier_status",
    ] {
        if value.get(key).is_none() {
            return Err(episode_jsonl_invalid(
                path,
                line,
                key,
                format!("episode row missing required field {key}"),
            ));
        }
    }
    match value["record_kind"].as_str() {
        Some("verified_agent_episode" | "incident_agent_episode") => {}
        Some(other) => {
            return Err(episode_jsonl_invalid(
                path,
                line,
                "record_kind",
                format!("unsupported episode record_kind {other}"),
            ));
        }
        None => {
            return Err(episode_jsonl_invalid(
                path,
                line,
                "record_kind",
                "record_kind must be string",
            ));
        }
    }
    match value["dataset_split"].as_str() {
        Some("passed" | "failed" | "blocked" | "incident" | "edge_case") => {}
        Some(other) => {
            return Err(episode_jsonl_invalid(
                path,
                line,
                "dataset_split",
                format!("unsupported dataset_split {other}"),
            ));
        }
        None => {
            return Err(episode_jsonl_invalid(
                path,
                line,
                "dataset_split",
                "dataset_split must be string",
            ));
        }
    }
    for pointer in [
        "/source/evidence_packet_id",
        "/source/ledger_sequence_range",
        "/source/ledger_tip_sha256_at_export",
        "/verifier_status/status",
    ] {
        if value.pointer(pointer).is_none() {
            return Err(episode_jsonl_invalid(
                path,
                line,
                pointer,
                format!("episode row missing required pointer {pointer}"),
            ));
        }
    }
    Ok(())
}

fn episode_jsonl_invalid(
    path: &Path,
    line: usize,
    field: impl Into<String>,
    message: impl Into<String>,
) -> BlackboxError {
    BlackboxError::evidence_integrity_for_connector(
        "EPISODE_JSONL_INVALID",
        FS_CONNECTOR_ID,
        file_source_of_truth(path),
        format!("episodes.jsonl:{line}:{}", field.into()),
        message,
        "regenerate the export and inspect the named JSONL field",
    )
}

fn write_eval_fixtures(evals_dir: &Path, rows: &[Value]) -> BbxResult<Vec<Value>> {
    let mut fixtures = Vec::new();
    for row in rows {
        let split = row
            .get("dataset_split")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        if split == "passed" {
            continue;
        }
        let error_code = row
            .get("error_code")
            .and_then(Value::as_str)
            .unwrap_or("UNKNOWN_ERROR");
        let episode_id = row
            .get("episode_id")
            .and_then(Value::as_str)
            .unwrap_or("episode_unknown");
        let file_name = format!(
            "{:04}_{}_{}.json",
            fixtures.len() + 1,
            safe_filename_fragment(split),
            safe_filename_fragment(error_code)
        );
        let path = evals_dir.join(file_name);
        let fixture = json!({
            "format": "blackbox_eval_fixture_json_v1",
            "schema_version": EPISODE_EXPORT_SCHEMA_VERSION,
            "generated_from_episode_id": episode_id,
            "dataset_split": split,
            "evidence_packet_id": row.pointer("/source/evidence_packet_id").cloned().unwrap_or(Value::Null),
            "source_of_truth": row.get("source").cloned().unwrap_or(Value::Null),
            "input": {
                "state": row.get("state").cloned().unwrap_or(Value::Null),
                "action": row.get("action").cloned().unwrap_or(Value::Null),
                "expected_persisted_delta": row.get("expected_persisted_delta").cloned().unwrap_or(Value::Null),
                "actual_readback": row.get("actual_readback").cloned().unwrap_or(Value::Null),
            },
            "expected": {
                "verifier_status": row.get("verifier_status").cloned().unwrap_or(Value::Null),
                "error_code": row.get("error_code").cloned().unwrap_or(Value::Null),
                "remediation": row.get("remediation").cloned().unwrap_or(Value::Null),
            }
        });
        write_json_file_new(&path, &fixture, "episode eval fixture")?;
        let readback = read_json_file(&path, "episode eval fixture readback")?;
        if readback["format"] != "blackbox_eval_fixture_json_v1" {
            return Err(BlackboxError::evidence_integrity_for_connector(
                "EPISODE_EVAL_READBACK_MISMATCH",
                FS_CONNECTOR_ID,
                file_source_of_truth(&path),
                "format",
                "eval fixture readback did not preserve expected format",
                "delete the corrupt export and rerun after inspecting filesystem/storage consistency",
            ));
        }
        fixtures.push(json!({
            "path": path,
            "sha256": sha256_file(&path)?,
            "bytes": fs::metadata(&path).map_err(|err| BlackboxError::io(&path, "stat eval fixture", err))?.len(),
            "dataset_split": split,
            "error_code": error_code,
            "episode_id": episode_id,
        }));
    }
    Ok(fixtures)
}

fn build_episodes_manifest(
    args: &ExportEpisodesArgs,
    inspection: &LedgerInspection,
    ledger_id: &str,
    episodes_path: &Path,
    readback: &EpisodeJsonlReadback,
    eval_fixtures: &[Value],
    sequence_range: (i64, i64),
) -> Value {
    json!({
        "status": "ok",
        "format": "blackbox_episodes_manifest_json_v1",
        "schema_version": EPISODE_EXPORT_SCHEMA_VERSION,
        "generated_at": Utc::now().to_rfc3339(),
        "source_ledger": {
            "path": args.ledger,
            "source_of_truth": source_of_truth(&args.ledger),
            "ledger_id": ledger_id,
            "ledger_schema_version": LEDGER_SCHEMA_VERSION,
            "ledger_record_count": inspection.record_count,
            "ledger_sequence_range": [sequence_range.0, sequence_range.1],
            "ledger_tip_sha256_at_export": inspection.ledger_tip_sha256,
        },
        "output": {
            "directory": args.out,
            "episodes_jsonl": episodes_path,
            "episodes_sha256": readback.sha256,
            "source_sha256": readback.sha256,
            "episodes_bytes": readback.size_bytes,
            "readback": readback,
        },
        "counts": {
            "episodes_total": readback.line_count,
            "rows_by_kind": readback.rows_by_kind,
            "splits": readback.splits,
        },
        "evals": {
            "directory": episodes_path.parent().unwrap_or_else(|| Path::new(".")).join("evals"),
            "fixture_count": eval_fixtures.len(),
            "fixtures": eval_fixtures,
        },
    })
}

fn write_json_file_new(path: &Path, value: &Value, label: &'static str) -> BbxResult<()> {
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|err| BlackboxError::io(path, label, err))?;
    let mut writer = BufWriter::new(file);
    serde_json::to_writer_pretty(&mut writer, value).map_err(|err| {
        BlackboxError::new_with_connector(
            ErrorKind::InvalidInput,
            "BBX_JSON_SERIALIZE_FAILED",
            format!("{label}: {err}"),
            Some(file_source_of_truth(path)),
            FS_CONNECTOR_ID,
            Some(path.display().to_string()),
            "inspect the export JSON value and rerun",
        )
    })?;
    writer
        .write_all(b"\n")
        .map_err(|err| BlackboxError::io(path, "write JSON file newline", err))?;
    writer
        .flush()
        .map_err(|err| BlackboxError::io(path, "flush JSON file", err))?;
    Ok(())
}

fn read_json_file(path: &Path, label: &'static str) -> BbxResult<Value> {
    let bytes = fs::read(path).map_err(|err| BlackboxError::io(path, label, err))?;
    serde_json::from_slice(&bytes).map_err(|err| {
        BlackboxError::evidence_integrity_for_connector(
            "BBX_JSON_READBACK_INVALID",
            FS_CONNECTOR_ID,
            file_source_of_truth(path),
            path.display().to_string(),
            format!("{label} does not parse as JSON after write: {err}"),
            "delete the corrupt export and rerun after inspecting filesystem/storage consistency",
        )
    })
}

fn sha256_file(path: &Path) -> BbxResult<String> {
    let bytes =
        fs::read(path).map_err(|err| BlackboxError::io(path, "read file for sha256", err))?;
    Ok(sha256_hex(&bytes))
}

fn export_temp_dir(path: &Path) -> PathBuf {
    PathBuf::from(format!(
        "{}.tmp-{}",
        path.display(),
        Uuid::new_v4().simple()
    ))
}

fn cleanup_export_temp_dir(path: &Path) -> BbxResult<()> {
    if path.exists() {
        fs::remove_dir_all(path).map_err(|err| {
            BlackboxError::io(path, "remove stale episode export temp directory", err)
        })?;
    }
    Ok(())
}

fn promote_episode_export_dir(temp_dir: &Path, out: &Path, force: bool) -> BbxResult<()> {
    if out.exists() {
        if !force {
            return Err(BlackboxError::new_with_connector(
                ErrorKind::InvalidInput,
                "BBX_OUTPUT_EXISTS",
                format!(
                    "episode export output directory already exists: {}",
                    out.display()
                ),
                Some(file_source_of_truth(out)),
                FS_CONNECTOR_ID,
                Some("--out".to_string()),
                "rerun with --force only when replacing this export directory is intentional",
            ));
        }
        fs::remove_dir_all(out).map_err(|err| {
            BlackboxError::io(out, "replace existing episode export directory", err)
        })?;
    }
    fs::rename(temp_dir, out)
        .map_err(|err| BlackboxError::io(out, "promote episode export temp directory", err))?;
    Ok(())
}

fn validate_dashboard_directory_path(path: &Path, force: bool) -> BbxResult<()> {
    if path.as_os_str().is_empty() {
        return Err(BlackboxError::new_with_connector(
            ErrorKind::InvalidInput,
            "BBX_EXPORT_PATH_INVALID",
            "--out must not be empty",
            None,
            FS_CONNECTOR_ID,
            Some("--out".to_string()),
            "provide a non-empty dashboard output directory",
        ));
    }
    if path.exists() {
        if !path.is_dir() {
            return Err(BlackboxError::new_with_connector(
                ErrorKind::InvalidInput,
                "BBX_EXPORT_PATH_INVALID",
                "--out exists and is not a directory",
                Some(file_source_of_truth(path)),
                FS_CONNECTOR_ID,
                Some("--out".to_string()),
                "provide a directory path or remove the existing file",
            ));
        }
        if !force {
            return Err(BlackboxError::new_with_connector(
                ErrorKind::InvalidInput,
                "BBX_OUTPUT_EXISTS",
                format!(
                    "dashboard output directory already exists: {}",
                    path.display()
                ),
                Some(file_source_of_truth(path)),
                FS_CONNECTOR_ID,
                Some("--out".to_string()),
                "rerun with --force only when replacing this dashboard directory is intentional",
            ));
        }
    }
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() && (!parent.exists() || !parent.is_dir()) {
            return Err(BlackboxError::new_with_connector(
                ErrorKind::InvalidInput,
                "BBX_EXPORT_PATH_INVALID",
                format!(
                    "dashboard parent directory does not exist: {}",
                    parent.display()
                ),
                Some(file_source_of_truth(path)),
                FS_CONNECTOR_ID,
                Some("--out".to_string()),
                "create the parent directory explicitly and rerun the dashboard build",
            ));
        }
    }
    Ok(())
}

fn cleanup_dashboard_temp_dir(path: &Path) -> BbxResult<()> {
    if path.exists() {
        fs::remove_dir_all(path)
            .map_err(|err| BlackboxError::io(path, "remove stale dashboard temp directory", err))?;
    }
    Ok(())
}

fn promote_dashboard_dir(temp_dir: &Path, out: &Path, force: bool) -> BbxResult<()> {
    if out.exists() {
        if !force {
            return Err(BlackboxError::new_with_connector(
                ErrorKind::InvalidInput,
                "BBX_OUTPUT_EXISTS",
                format!(
                    "dashboard output directory already exists: {}",
                    out.display()
                ),
                Some(file_source_of_truth(out)),
                FS_CONNECTOR_ID,
                Some("--out".to_string()),
                "rerun with --force only when replacing this dashboard directory is intentional",
            ));
        }
        fs::remove_dir_all(out)
            .map_err(|err| BlackboxError::io(out, "replace existing dashboard directory", err))?;
    }
    fs::rename(temp_dir, out)
        .map_err(|err| BlackboxError::io(out, "promote dashboard temp directory", err))?;
    Ok(())
}

fn write_dashboard_json_file(path: &Path, value: &Value) -> BbxResult<()> {
    write_json_file_new(path, value, "dashboard API JSON")?;
    let readback = read_json_file(path, "dashboard API JSON readback")?;
    if &readback != value {
        return Err(BlackboxError::evidence_integrity_for_connector(
            "DASHBOARD_API_READBACK_MISMATCH",
            FS_CONNECTOR_ID,
            file_source_of_truth(path),
            path.display().to_string(),
            "dashboard API JSON readback does not match the value written",
            "delete the corrupt dashboard directory and rerun after inspecting filesystem/storage consistency",
        ));
    }
    Ok(())
}

fn write_dashboard_text_file(path: &Path, text: &str, label: &'static str) -> BbxResult<()> {
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|err| BlackboxError::io(path, label, err))?;
    let mut writer = BufWriter::new(file);
    writer
        .write_all(text.as_bytes())
        .map_err(|err| BlackboxError::io(path, label, err))?;
    writer
        .flush()
        .map_err(|err| BlackboxError::io(path, "flush dashboard text file", err))?;
    let readback = fs::read_to_string(path)
        .map_err(|err| BlackboxError::io(path, "read dashboard HTML", err))?;
    if readback != text {
        return Err(BlackboxError::evidence_integrity_for_connector(
            "DASHBOARD_HTML_READBACK_MISMATCH",
            FS_CONNECTOR_ID,
            file_source_of_truth(path),
            path.display().to_string(),
            "dashboard HTML readback does not match the value written",
            "delete the corrupt dashboard directory and rerun after inspecting filesystem/storage consistency",
        ));
    }
    Ok(())
}

fn dashboard_file_entry(base: &Path, relative: &Path) -> BbxResult<Value> {
    let path = base.join(relative);
    let bytes = fs::metadata(&path)
        .map_err(|err| BlackboxError::io(&path, "stat dashboard file", err))?
        .len();
    Ok(json!({
        "path": relative.to_string_lossy(),
        "sha256": sha256_file(&path)?,
        "bytes": bytes,
    }))
}

fn verify_dashboard_manifest_files(out: &Path, manifest: &Value) -> BbxResult<()> {
    let files = manifest
        .get("files")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            BlackboxError::evidence_integrity_for_connector(
                "DASHBOARD_MANIFEST_READBACK_MISMATCH",
                FS_CONNECTOR_ID,
                file_source_of_truth(&out.join("manifest.json")),
                "manifest.json.files",
                "dashboard manifest is missing files array",
                "delete the corrupt dashboard directory and rerun after inspecting filesystem/storage consistency",
            )
        })?;
    for file in files {
        let rel = file.get("path").and_then(Value::as_str).ok_or_else(|| {
            BlackboxError::evidence_integrity_for_connector(
                "DASHBOARD_MANIFEST_READBACK_MISMATCH",
                FS_CONNECTOR_ID,
                file_source_of_truth(&out.join("manifest.json")),
                "manifest.json.files[].path",
                "dashboard manifest file entry is missing path",
                "delete the corrupt dashboard directory and rerun after inspecting filesystem/storage consistency",
            )
        })?;
        let rel_path = Path::new(rel);
        if rel_path.is_absolute()
            || rel_path
                .components()
                .any(|component| matches!(component, std::path::Component::ParentDir))
        {
            return Err(BlackboxError::evidence_integrity_for_connector(
                "DASHBOARD_MANIFEST_PATH_INVALID",
                FS_CONNECTOR_ID,
                file_source_of_truth(&out.join("manifest.json")),
                "manifest.json.files[].path",
                format!("dashboard manifest file path is not a safe relative path: {rel}"),
                "regenerate the dashboard manifest from trusted code",
            ));
        }
        let path = out.join(rel_path);
        let expected_sha = file.get("sha256").and_then(Value::as_str).ok_or_else(|| {
            BlackboxError::evidence_integrity_for_connector(
                "DASHBOARD_MANIFEST_READBACK_MISMATCH",
                FS_CONNECTOR_ID,
                file_source_of_truth(&out.join("manifest.json")),
                "manifest.json.files[].sha256",
                "dashboard manifest file entry is missing sha256",
                "delete the corrupt dashboard directory and rerun after inspecting filesystem/storage consistency",
            )
        })?;
        let expected_bytes = file.get("bytes").and_then(Value::as_u64).ok_or_else(|| {
            BlackboxError::evidence_integrity_for_connector(
                "DASHBOARD_MANIFEST_READBACK_MISMATCH",
                FS_CONNECTOR_ID,
                file_source_of_truth(&out.join("manifest.json")),
                "manifest.json.files[].bytes",
                "dashboard manifest file entry is missing byte count",
                "delete the corrupt dashboard directory and rerun after inspecting filesystem/storage consistency",
            )
        })?;
        let actual_sha = sha256_file(&path)?;
        let actual_bytes = fs::metadata(&path)
            .map_err(|err| BlackboxError::io(&path, "stat final dashboard file", err))?
            .len();
        if actual_sha != expected_sha || actual_bytes != expected_bytes {
            return Err(BlackboxError::evidence_integrity_for_connector(
                "DASHBOARD_FILE_HASH_MISMATCH",
                FS_CONNECTOR_ID,
                file_source_of_truth(&path),
                rel,
                "dashboard file hash or byte count changed after promotion",
                "treat the dashboard output directory as unreliable and inspect filesystem/storage before retrying",
            ));
        }
    }
    Ok(())
}

fn ledger_record_kind_counts(records: &[LedgerRecordOutput]) -> BTreeMap<String, i64> {
    let mut counts = BTreeMap::new();
    for record in records {
        *counts.entry(record.record_kind.clone()).or_insert(0) += 1;
    }
    counts
}

fn ledger_record_kind_counts_from_refs(records: &[&LedgerRecordOutput]) -> BTreeMap<String, i64> {
    let mut counts = BTreeMap::new();
    for record in records {
        *counts.entry(record.record_kind.clone()).or_insert(0) += 1;
    }
    counts
}

fn verifier_status_counts(
    ledger_path: &Path,
    records: &[LedgerRecordOutput],
) -> BbxResult<BTreeMap<String, i64>> {
    let mut counts = initialized_status_counts();
    for record in records
        .iter()
        .filter(|record| record.record_kind == "verifier_result")
    {
        let status = required_dashboard_payload_str(ledger_path, record, "status")?;
        *counts.entry(status.to_string()).or_insert(0) += 1;
    }
    Ok(counts)
}

fn verifier_status_counts_from_refs(
    ledger_path: &Path,
    records: &[&LedgerRecordOutput],
) -> BbxResult<BTreeMap<String, i64>> {
    let mut counts = initialized_status_counts();
    for record in records
        .iter()
        .filter(|record| record.record_kind == "verifier_result")
    {
        let status = required_dashboard_payload_str(ledger_path, record, "status")?;
        *counts.entry(status.to_string()).or_insert(0) += 1;
    }
    Ok(counts)
}

fn initialized_status_counts() -> BTreeMap<String, i64> {
    let mut counts = BTreeMap::new();
    for status in ["passed", "failed", "blocked", "unknown"] {
        counts.insert(status.to_string(), 0);
    }
    counts
}

fn policy_decision_counts(
    ledger_path: &Path,
    records: &[LedgerRecordOutput],
) -> BbxResult<BTreeMap<String, i64>> {
    let mut counts = BTreeMap::new();
    for status in ["allowed", "denied", "unknown"] {
        counts.insert(status.to_string(), 0);
    }
    for record in records
        .iter()
        .filter(|record| record.record_kind == "policy_decision")
    {
        let status = required_dashboard_payload_str(ledger_path, record, "status")?;
        *counts.entry(status.to_string()).or_insert(0) += 1;
    }
    Ok(counts)
}

fn policy_decision_counts_from_refs(
    ledger_path: &Path,
    records: &[&LedgerRecordOutput],
) -> BbxResult<BTreeMap<String, i64>> {
    let mut counts = BTreeMap::new();
    for status in ["allowed", "denied", "unknown"] {
        counts.insert(status.to_string(), 0);
    }
    for record in records
        .iter()
        .filter(|record| record.record_kind == "policy_decision")
    {
        let status = required_dashboard_payload_str(ledger_path, record, "status")?;
        *counts.entry(status.to_string()).or_insert(0) += 1;
    }
    Ok(counts)
}

fn records_by_run_id(records: &[LedgerRecordOutput]) -> BTreeMap<String, Vec<&LedgerRecordOutput>> {
    let mut by_run = BTreeMap::<String, Vec<&LedgerRecordOutput>>::new();
    for record in records {
        if let Some(run_id) = &record.run_id {
            by_run.entry(run_id.clone()).or_default().push(record);
        }
    }
    by_run
}

fn tool_call_records_by_id<'a>(
    ledger_path: &Path,
    records: &'a [LedgerRecordOutput],
) -> BbxResult<BTreeMap<String, &'a LedgerRecordOutput>> {
    let mut by_tool = BTreeMap::new();
    for record in records
        .iter()
        .filter(|record| record.record_kind == "tool_call")
    {
        let tool_call_id = required_dashboard_payload_str(ledger_path, record, "tool_call_id")?;
        if by_tool.insert(tool_call_id.to_string(), record).is_some() {
            return Err(dashboard_schema_error(
                ledger_path,
                record,
                "payload.tool_call_id",
                format!("duplicate tool_call_id {tool_call_id}"),
            ));
        }
    }
    Ok(by_tool)
}

fn evidence_tool_call_ids(
    ledger_path: &Path,
    records: &[LedgerRecordOutput],
) -> BbxResult<BTreeSet<String>> {
    let mut ids = BTreeSet::new();
    for record in records
        .iter()
        .filter(|record| record.record_kind == "evidence_packet")
    {
        ids.insert(
            required_dashboard_payload_str(ledger_path, record, "tool_call_id")?.to_string(),
        );
    }
    Ok(ids)
}

fn dashboard_run_status(
    verifier_counts: &BTreeMap<String, i64>,
    policy_counts: &BTreeMap<String, i64>,
    incident_packets_total: i64,
    tool_calls_total: i64,
    evidence_packets_total: i64,
) -> &'static str {
    if incident_packets_total > 0
        || verifier_counts.get("failed").copied().unwrap_or(0) > 0
        || policy_counts.get("denied").copied().unwrap_or(0) > 0
    {
        "failed"
    } else if verifier_counts.get("blocked").copied().unwrap_or(0) > 0 {
        "blocked"
    } else if tool_calls_total > evidence_packets_total {
        "needs_verification"
    } else if verifier_counts.get("passed").copied().unwrap_or(0) > 0 {
        "passed"
    } else {
        "recorded"
    }
}

fn increment_nested_count(
    counts: &mut BTreeMap<String, BTreeMap<String, i64>>,
    dimension: &str,
    status: &str,
) {
    let entry = counts.entry(dimension.to_string()).or_default();
    for known in ["passed", "failed", "blocked", "unknown"] {
        entry.entry(known.to_string()).or_insert(0);
    }
    *entry.entry(status.to_string()).or_insert(0) += 1;
}

fn ratio_value(numerator: i64, denominator: i64) -> Value {
    if denominator <= 0 {
        Value::Null
    } else {
        json!((numerator as f64) / (denominator as f64))
    }
}

fn dashboard_source_ledger(
    ledger_path: &Path,
    inspection: &LedgerInspection,
    ledger_id: &str,
) -> Value {
    json!({
        "path": ledger_path,
        "source_of_truth": source_of_truth(ledger_path),
        "ledger_id": ledger_id,
        "ledger_schema_version": LEDGER_SCHEMA_VERSION,
        "ledger_record_count": inspection.record_count,
        "ledger_tip_sha256": inspection.ledger_tip_sha256,
    })
}

fn dashboard_timeline_file_name(run_id: &str) -> String {
    format!("timeline_{}.json", safe_filename_fragment(run_id))
}

fn required_dashboard_payload_str<'a>(
    ledger_path: &Path,
    record: &'a LedgerRecordOutput,
    field: &'static str,
) -> BbxResult<&'a str> {
    record
        .payload
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            dashboard_schema_error(
                ledger_path,
                record,
                format!("payload.{field}"),
                format!(
                    "{} payload is missing non-empty {field}",
                    record.record_kind
                ),
            )
        })
}

fn required_dashboard_actor_str<'a>(
    ledger_path: &Path,
    record: &'a LedgerRecordOutput,
    field: &'static str,
) -> BbxResult<&'a str> {
    record
        .actor
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            dashboard_schema_error(
                ledger_path,
                record,
                format!("actor.{field}"),
                format!("{} actor is missing non-empty {field}", record.record_kind),
            )
        })
}

fn dashboard_schema_error(
    ledger_path: &Path,
    record: &LedgerRecordOutput,
    field: impl Into<String>,
    message: impl Into<String>,
) -> BlackboxError {
    BlackboxError::evidence_integrity(
        "DASHBOARD_SOURCE_SCHEMA_INVALID",
        source_of_truth(ledger_path),
        format!("ledger_records[{}].{}", record.sequence, field.into()),
        message,
        "inspect the physical ledger row and rebuild the invalid record from a verified run",
    )
}

#[derive(Debug)]
struct DashboardErrorGroup {
    error_code: String,
    count: i64,
    record_kinds: BTreeSet<String>,
    tools: BTreeSet<String>,
    remediations: BTreeSet<String>,
    source_examples: Vec<Value>,
    first_sequence: i64,
    last_sequence: i64,
}

impl DashboardErrorGroup {
    fn new(error_code: &str) -> Self {
        Self {
            error_code: error_code.to_string(),
            count: 0,
            record_kinds: BTreeSet::new(),
            tools: BTreeSet::new(),
            remediations: BTreeSet::new(),
            source_examples: Vec::new(),
            first_sequence: i64::MAX,
            last_sequence: 0,
        }
    }

    fn to_json(self) -> Value {
        json!({
            "error_code": self.error_code,
            "count": self.count,
            "record_kinds": self.record_kinds,
            "tools": self.tools,
            "remediations": self.remediations,
            "source_of_truth_examples": self.source_examples,
            "ledger_sequence_range": [self.first_sequence, self.last_sequence],
        })
    }
}

fn build_dashboard_html(data: &DashboardData) -> BbxResult<String> {
    let summary = &data.summary;
    let ledger_source = html_escape(
        summary
            .pointer("/source_ledger/source_of_truth")
            .and_then(Value::as_str)
            .unwrap_or(""),
    );
    let mut html = String::new();
    html.push_str("<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n");
    html.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");
    html.push_str("<title>BlackBoxFix Team Dashboard</title>\n");
    html.push_str("<style>\n");
    html.push_str(":root{color-scheme:light;--ink:#17212b;--muted:#5a6673;--line:#c9d1d9;--panel:#f7f9fb;--ok:#1b7f3a;--fail:#b42318;--warn:#9a5b00;--bg:#ffffff;}\n");
    html.push_str("*{box-sizing:border-box}body{margin:0;background:var(--bg);color:var(--ink);font:14px/1.45 system-ui,-apple-system,BlinkMacSystemFont,\"Segoe UI\",sans-serif}header,main{max-width:1280px;margin:0 auto;padding:20px}header{border-bottom:1px solid var(--line)}h1{font-size:24px;margin:0 0 8px}h2{font-size:18px;margin:24px 0 10px}.source{color:var(--muted);word-break:break-all}nav{display:flex;gap:10px;flex-wrap:wrap;margin-top:12px}a{color:#075985}table{width:100%;border-collapse:collapse;table-layout:fixed;background:#fff}th,td{border:1px solid var(--line);padding:8px;vertical-align:top;overflow-wrap:anywhere}th{background:var(--panel);text-align:left}.metric-grid{display:grid;grid-template-columns:repeat(auto-fit,minmax(160px,1fr));gap:8px}.metric{border:1px solid var(--line);padding:10px;background:var(--panel);min-height:70px}.metric b{display:block;font-size:22px}.filters{display:flex;gap:10px;flex-wrap:wrap;align-items:end;margin:0 0 10px}.filters label{display:grid;gap:4px;color:var(--muted)}select,input{border:1px solid var(--line);background:#fff;color:var(--ink);min-height:34px;padding:5px 8px;min-width:150px}.status-passed{color:var(--ok);font-weight:700}.status-failed{color:var(--fail);font-weight:700}.status-blocked,.status-needs_verification{color:var(--warn);font-weight:700}code{font-family:ui-monospace,SFMono-Regular,Consolas,monospace;font-size:12px}@media(max-width:760px){header,main{padding:14px}table{font-size:12px}.wide{overflow-x:auto}th,td{min-width:130px}select,input{width:100%}.filters label{width:100%}}\n");
    html.push_str("</style>\n</head>\n<body>\n<header>\n");
    html.push_str("<h1>BlackBoxFix Team Dashboard</h1>\n");
    html.push_str(&format!(
        "<div class=\"source\">Source of truth: <code>{}</code></div>\n",
        ledger_source
    ));
    html.push_str("<nav aria-label=\"Dashboard API responses\"><a href=\"api/summary.json\">summary</a><a href=\"api/runs.json\">runs</a><a href=\"api/evidence.json\">evidence</a><a href=\"api/verifier-status.json\">verifiers</a><a href=\"api/errors.json\">errors</a><a href=\"api/exports.json\">exports</a></nav>\n");
    html.push_str("</header>\n<main>\n<div role=\"status\">Dashboard files read back and hash verified.</div>\n");
    html.push_str("<section aria-labelledby=\"metrics\"><h2 id=\"metrics\">Metrics</h2><div class=\"metric-grid\">\n");
    for metric in dashboard_array(summary, "/metrics")? {
        let label = html_escape(metric.get("label").and_then(Value::as_str).unwrap_or(""));
        let value = html_escape(&dashboard_display_value(
            metric.get("value").unwrap_or(&Value::Null),
        ));
        html.push_str(&format!(
            "<div class=\"metric\"><span>{}</span><b>{}</b></div>\n",
            label, value
        ));
    }
    html.push_str("</div></section>\n");

    html.push_str("<section aria-labelledby=\"runs\"><h2 id=\"runs\">Runs</h2><div class=\"wide\"><table><thead><tr><th>Run</th><th>Status</th><th>Task</th><th>Verifier Status</th><th>Source</th><th>Timeline</th></tr></thead><tbody>\n");
    for run in dashboard_array(&data.runs, "/runs")? {
        let run_id = html_escape(run.get("run_id").and_then(Value::as_str).unwrap_or(""));
        let status = run.get("status").and_then(Value::as_str).unwrap_or("");
        let class = format!("status-{}", safe_filename_fragment(status));
        let task = html_escape(&dashboard_display_value(
            run.get("task").unwrap_or(&Value::Null),
        ));
        let verifier = html_escape(&dashboard_display_value(
            run.get("verifier_status_counts").unwrap_or(&Value::Null),
        ));
        let source = html_escape(&dashboard_display_value(
            run.get("source_of_truth").unwrap_or(&Value::Null),
        ));
        let timeline = html_escape(
            run.get("timeline_api")
                .and_then(Value::as_str)
                .unwrap_or(""),
        );
        html.push_str(&format!(
            "<tr><td><code>{}</code></td><td class=\"{}\">{}</td><td>{}</td><td><code>{}</code></td><td><code>{}</code></td><td><a href=\"{}\">timeline</a></td></tr>\n",
            run_id,
            class,
            html_escape(status),
            task,
            verifier,
            source,
            timeline
        ));
    }
    html.push_str("</tbody></table></div></section>\n");

    html.push_str("<section aria-labelledby=\"timeline\"><h2 id=\"timeline\">Timeline</h2><div class=\"filters\" aria-label=\"Timeline filters\"><label>Record<select id=\"timeline-kind\"><option value=\"\">All</option><option value=\"agent_run\">agent_run</option><option value=\"agent_step\">agent_step</option><option value=\"model_decision\">model_decision</option><option value=\"policy_decision\">policy_decision</option><option value=\"tool_call\">tool_call</option><option value=\"expected_delta\">expected_delta</option><option value=\"readback_snapshot\">readback_snapshot</option><option value=\"verifier_result\">verifier_result</option><option value=\"evidence_packet\">evidence_packet</option><option value=\"incident_packet\">incident_packet</option></select></label><label>Status<select id=\"timeline-status\"><option value=\"\">All</option><option value=\"passed\">passed</option><option value=\"failed\">failed</option><option value=\"blocked\">blocked</option><option value=\"denied\">denied</option><option value=\"ok\">ok</option><option value=\"running\">running</option></select></label><label>Risk<select id=\"timeline-risk\"><option value=\"\">All</option><option value=\"low\">low</option><option value=\"medium\">medium</option><option value=\"high\">high</option></select></label><label>Source<input id=\"timeline-source\" type=\"search\" autocomplete=\"off\"></label></div><div class=\"wide\"><table><thead><tr><th>Run</th><th>Seq</th><th>Record</th><th>Status</th><th>Risk</th><th>Source Of Truth</th><th>Detail</th></tr></thead><tbody>\n");
    for (_, _, timeline) in &data.timelines {
        let run_id = timeline.get("run_id").and_then(Value::as_str).unwrap_or("");
        for record in dashboard_array(timeline, "/report/timeline")? {
            let record_kind = record
                .get("record_kind")
                .and_then(Value::as_str)
                .unwrap_or("");
            let status = dashboard_timeline_record_status(record);
            let risk = dashboard_timeline_record_risk(record);
            let source = dashboard_timeline_record_source(record);
            let detail = dashboard_timeline_record_detail(record);
            html.push_str(&format!(
                "<tr class=\"timeline-row\" data-kind=\"{}\" data-status=\"{}\" data-risk=\"{}\" data-source=\"{}\"><td><code>{}</code></td><td>{}</td><td><code>{}</code></td><td class=\"status-{}\">{}</td><td>{}</td><td><code>{}</code></td><td>{}</td></tr>\n",
                html_escape(record_kind),
                html_escape(&status),
                html_escape(&risk),
                html_escape(&source),
                html_escape(run_id),
                html_escape(&dashboard_display_value(record.get("sequence").unwrap_or(&Value::Null))),
                html_escape(record_kind),
                safe_filename_fragment(&status),
                html_escape(&status),
                html_escape(&risk),
                html_escape(&source),
                html_escape(&detail)
            ));
        }
    }
    html.push_str("</tbody></table></div></section>\n");

    html.push_str("<section aria-labelledby=\"verifiers\"><h2 id=\"verifiers\">Verifier Status</h2><div class=\"wide\"><table><thead><tr><th>Verifier</th><th>Status</th><th>Tool</th><th>Error</th><th>Source Of Truth</th></tr></thead><tbody>\n");
    for verifier in dashboard_array(&data.verifiers, "/verifier_results")? {
        let status = verifier.get("status").and_then(Value::as_str).unwrap_or("");
        html.push_str(&format!(
            "<tr><td><code>{}</code></td><td class=\"status-{}\">{}</td><td>{}</td><td><code>{}</code></td><td><code>{}</code></td></tr>\n",
            html_escape(verifier.get("verifier_id").and_then(Value::as_str).unwrap_or("")),
            safe_filename_fragment(status),
            html_escape(status),
            html_escape(verifier.get("tool_name").and_then(Value::as_str).unwrap_or("")),
            html_escape(&dashboard_display_value(verifier.get("error_code").unwrap_or(&Value::Null))),
            html_escape(&dashboard_display_value(verifier.get("source_of_truth").unwrap_or(&Value::Null)))
        ));
    }
    html.push_str("</tbody></table></div></section>\n");

    html.push_str("<section aria-labelledby=\"evidence\"><h2 id=\"evidence\">Evidence</h2><div class=\"wide\"><table><thead><tr><th>Evidence Packet</th><th>Run</th><th>Tool Call</th><th>Status</th><th>Record SHA256</th><th>Source Of Truth</th></tr></thead><tbody>\n");
    for packet in dashboard_array(&data.evidence, "/evidence_packets")? {
        html.push_str(&format!(
            "<tr><td><code>{}</code></td><td><code>{}</code></td><td><code>{}</code></td><td>{}</td><td><code>{}</code></td><td><code>{}</code></td></tr>\n",
            html_escape(packet.get("evidence_packet_id").and_then(Value::as_str).unwrap_or("")),
            html_escape(packet.get("run_id").and_then(Value::as_str).unwrap_or("")),
            html_escape(packet.get("tool_call_id").and_then(Value::as_str).unwrap_or("")),
            html_escape(packet.get("verifier_status").and_then(Value::as_str).unwrap_or("")),
            html_escape(&dashboard_display_value(packet.get("record_sha256").unwrap_or(&Value::Null))),
            html_escape(&dashboard_display_value(packet.get("source_of_truth").unwrap_or(&Value::Null)))
        ));
    }
    html.push_str("</tbody></table></div></section>\n");

    html.push_str("<section aria-labelledby=\"errors\"><h2 id=\"errors\">Error Codes</h2><div class=\"wide\"><table><thead><tr><th>Error Code</th><th>Count</th><th>Records</th><th>Remediation</th><th>Source Of Truth</th></tr></thead><tbody>\n");
    for error in dashboard_array(&data.errors, "/error_codes")? {
        html.push_str(&format!(
            "<tr><td><code>{}</code></td><td>{}</td><td><code>{}</code></td><td>{}</td><td><code>{}</code></td></tr>\n",
            html_escape(error.get("error_code").and_then(Value::as_str).unwrap_or("")),
            html_escape(&dashboard_display_value(error.get("count").unwrap_or(&Value::Null))),
            html_escape(&dashboard_display_value(error.get("record_kinds").unwrap_or(&Value::Null))),
            html_escape(&dashboard_display_value(error.get("remediations").unwrap_or(&Value::Null))),
            html_escape(&dashboard_display_value(error.get("source_of_truth_examples").unwrap_or(&Value::Null)))
        ));
    }
    html.push_str("</tbody></table></div></section>\n");

    html.push_str("<section aria-labelledby=\"exports\"><h2 id=\"exports\">Exports</h2><div class=\"wide\"><table><thead><tr><th>Run</th><th>Evidence Command</th><th>Timeline API</th></tr></thead><tbody>\n");
    for export in dashboard_array(&data.exports, "/exports/evidence_by_run")? {
        html.push_str(&format!(
            "<tr><td><code>{}</code></td><td><code>{}</code></td><td><a href=\"{}\">timeline</a></td></tr>\n",
            html_escape(export.get("run_id").and_then(Value::as_str).unwrap_or("")),
            html_escape(&dashboard_display_value(export.get("evidence_export_command").unwrap_or(&Value::Null))),
            html_escape(export.get("timeline_api").and_then(Value::as_str).unwrap_or(""))
        ));
    }
    html.push_str("</tbody></table></div></section>\n");
    html.push_str("</main>\n<script>\n(function(){function q(id){return document.getElementById(id)}function apply(){var kind=q('timeline-kind').value;var status=q('timeline-status').value;var risk=q('timeline-risk').value;var source=q('timeline-source').value.toLowerCase();document.querySelectorAll('.timeline-row').forEach(function(row){var hide=(kind&&row.dataset.kind!==kind)||(status&&row.dataset.status!==status)||(risk&&row.dataset.risk!==risk)||(source&&row.dataset.source.toLowerCase().indexOf(source)===-1);row.hidden=hide;});}['timeline-kind','timeline-status','timeline-risk','timeline-source'].forEach(function(id){q(id).addEventListener('input',apply);});})();\n</script>\n</body>\n</html>\n");
    Ok(html)
}

fn dashboard_array<'a>(value: &'a Value, pointer: &'static str) -> BbxResult<&'a Vec<Value>> {
    value
        .pointer(pointer)
        .and_then(Value::as_array)
        .ok_or_else(|| {
            BlackboxError::invalid(
                format!("dashboard HTML source is missing array at {pointer}"),
                "report this as a bug; dashboard API builders must provide every rendered array",
            )
        })
}

fn dashboard_display_value(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

fn dashboard_timeline_record_status(record: &Value) -> String {
    for pointer in [
        "/payload/status",
        "/payload/verifier_status",
        "/payload/execution/status",
        "/payload/trigger",
    ] {
        if let Some(status) = record.pointer(pointer).and_then(Value::as_str) {
            if !status.trim().is_empty() {
                return status.to_string();
            }
        }
    }
    String::new()
}

fn dashboard_timeline_record_risk(record: &Value) -> String {
    record
        .pointer("/payload/task/risk_tier")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn dashboard_timeline_record_source(record: &Value) -> String {
    if let Some(source) = record.pointer("/payload/source_of_truth") {
        return dashboard_display_value(source);
    }
    if let Some(source) = record.pointer("/payload/source_of_truth/locator") {
        return dashboard_display_value(source);
    }
    if let Some(source) = record.pointer("/payload/read_path/connector_id") {
        return dashboard_display_value(source);
    }
    String::new()
}

fn dashboard_timeline_record_detail(record: &Value) -> String {
    for pointer in [
        "/payload/task/requested_outcome",
        "/payload/summary",
        "/payload/tool_name",
        "/payload/verifier_id",
        "/payload/error_code",
        "/payload/evidence_packet_id",
        "/payload/incident_packet_id",
    ] {
        if let Some(value) = record.pointer(pointer) {
            let rendered = dashboard_display_value(value);
            if !rendered.trim().is_empty() {
                return rendered;
            }
        }
    }
    dashboard_display_value(record.get("payload").unwrap_or(&Value::Null))
}

fn html_escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

fn safe_filename_fragment(value: &str) -> String {
    let mut out = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    if out.is_empty() {
        out = "unknown".to_string();
    }
    out.truncate(80);
    out
}

fn build_run_replay_report(
    ledger_path: &Path,
    inspection: &LedgerInspection,
    run_id: &str,
) -> BbxResult<Value> {
    let records = inspection
        .records
        .iter()
        .filter(|record| record.run_id.as_deref() == Some(run_id))
        .collect::<Vec<_>>();
    if records.is_empty() {
        return Err(BlackboxError::not_found(
            ledger_path,
            format!("run_id {run_id} does not exist in ledger_records"),
        ));
    }
    if !records
        .iter()
        .any(|record| record.record_kind.as_str() == "agent_run")
    {
        return Err(BlackboxError::evidence_integrity(
            "REPLAY_RUN_RECORD_MISSING",
            source_of_truth(ledger_path),
            "ledger_records.record_kind",
            format!("run_id {run_id} has no agent_run record"),
            "treat the ledger as inconsistent and inspect the append-only chain",
        ));
    }

    let mut kinds = BTreeMap::<String, i64>::new();
    let mut verifier_statuses = BTreeMap::<String, i64>::new();
    let mut first_failure = Value::Null;
    for record in &records {
        *kinds.entry(record.record_kind.clone()).or_insert(0) += 1;
        if record.record_kind == "verifier_result" {
            let status = record
                .payload
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .to_string();
            *verifier_statuses.entry(status.clone()).or_insert(0) += 1;
            if first_failure.is_null() && status != "passed" {
                first_failure = json!({
                    "sequence": record.sequence,
                    "verifier_id": record.payload.get("verifier_id").cloned().unwrap_or(Value::Null),
                    "status": status,
                    "error_code": record.payload.get("error_code").cloned().unwrap_or(Value::Null),
                    "source_of_truth": record.payload.get("source_of_truth").cloned().unwrap_or(Value::Null),
                    "remediation": record.payload.get("remediation").cloned().unwrap_or(Value::Null),
                });
            }
        }
    }

    let evidence_verification = verify_evidence_packet_refs(ledger_path, &records)?;
    let timeline = records
        .iter()
        .map(|record| {
            json!({
                "sequence": record.sequence,
                "record_id": record.record_id,
                "record_kind": record.record_kind,
                "created_at": record.created_at,
                "actor": record.actor,
                "payload": record.payload,
                "hash": record.hash,
                "redaction": record.redaction,
            })
        })
        .collect::<Vec<_>>();
    let incidents = records
        .iter()
        .filter(|record| record.record_kind == "incident_packet")
        .map(|record| {
            json!({
                "sequence": record.sequence,
                "record_id": record.record_id,
                "payload": record.payload,
                "hash": record.hash,
            })
        })
        .collect::<Vec<_>>();

    Ok(json!({
        "run_id": run_id,
        "mode": "forensic",
        "mutates_source_of_truth": false,
        "ledger_integrity": {
            "status": "passed",
            "schema_version": LEDGER_SCHEMA_VERSION,
            "record_count": inspection.record_count,
            "ledger_tip_sha256": inspection.ledger_tip_sha256,
        },
        "summary": {
            "record_count": records.len(),
            "kinds": kinds,
            "verifier_statuses": verifier_statuses,
            "first_failure": first_failure,
        },
        "evidence_hash_verification": evidence_verification,
        "timeline": timeline,
        "incidents": incidents,
    }))
}

fn verify_evidence_packet_refs(
    ledger_path: &Path,
    records: &[&LedgerRecordOutput],
) -> BbxResult<Value> {
    let mut checked_packets = 0_i64;
    let mut checked_refs = 0_i64;
    for packet in records
        .iter()
        .filter(|record| record.record_kind == "evidence_packet")
    {
        checked_packets += 1;
        let refs = packet
            .payload
            .get("record_refs")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                BlackboxError::evidence_integrity(
                    "EVIDENCE_PACKET_SCHEMA_INVALID",
                    source_of_truth(ledger_path),
                    format!("ledger_records[{}].payload.record_refs", packet.sequence),
                    "evidence_packet payload is missing record_refs array",
                    "rebuild the evidence packet from immutable ledger records",
                )
            })?;
        for (index, record_ref) in refs.iter().enumerate() {
            let sequence = record_ref
                .get("sequence")
                .and_then(Value::as_i64)
                .ok_or_else(|| {
                    BlackboxError::evidence_integrity(
                        "EVIDENCE_PACKET_SCHEMA_INVALID",
                        source_of_truth(ledger_path),
                        format!(
                            "ledger_records[{}].payload.record_refs[{index}].sequence",
                            packet.sequence
                        ),
                        "evidence record ref is missing integer sequence",
                        "rebuild the evidence packet from immutable ledger records",
                    )
                })?;
            let expected_kind = record_ref
                .get("record_kind")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    BlackboxError::evidence_integrity(
                        "EVIDENCE_PACKET_SCHEMA_INVALID",
                        source_of_truth(ledger_path),
                        format!(
                            "ledger_records[{}].payload.record_refs[{index}].record_kind",
                            packet.sequence
                        ),
                        "evidence record ref is missing record_kind",
                        "rebuild the evidence packet from immutable ledger records",
                    )
                })?;
            let expected_payload_sha256 = record_ref
                .get("payload_sha256")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    BlackboxError::evidence_integrity(
                        "EVIDENCE_PACKET_SCHEMA_INVALID",
                        source_of_truth(ledger_path),
                        format!(
                            "ledger_records[{}].payload.record_refs[{index}].payload_sha256",
                            packet.sequence
                        ),
                        "evidence record ref is missing payload_sha256",
                        "rebuild the evidence packet from immutable ledger records",
                    )
                })?;
            let expected_record_sha256 = record_ref
                .get("record_sha256")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    BlackboxError::evidence_integrity(
                        "EVIDENCE_PACKET_SCHEMA_INVALID",
                        source_of_truth(ledger_path),
                        format!(
                            "ledger_records[{}].payload.record_refs[{index}].record_sha256",
                            packet.sequence
                        ),
                        "evidence record ref is missing record_sha256",
                        "rebuild the evidence packet from immutable ledger records",
                    )
                })?;
            let Some(actual) = records.iter().find(|record| record.sequence == sequence) else {
                return Err(BlackboxError::evidence_integrity(
                    "EVIDENCE_HASH_MISMATCH",
                    source_of_truth(ledger_path),
                    format!(
                        "ledger_records[{}].payload.record_refs[{index}]",
                        packet.sequence
                    ),
                    format!("evidence references missing ledger sequence {sequence}"),
                    "treat the evidence packet as invalid and rebuild from an intact ledger",
                ));
            };
            let actual_payload_sha256 = actual
                .hash
                .get("payload_sha256")
                .and_then(Value::as_str)
                .unwrap_or("");
            let actual_record_sha256 = actual
                .hash
                .get("record_sha256")
                .and_then(Value::as_str)
                .unwrap_or("");
            if actual.record_kind != expected_kind
                || actual_payload_sha256 != expected_payload_sha256
                || actual_record_sha256 != expected_record_sha256
            {
                return Err(BlackboxError::evidence_integrity(
                    "EVIDENCE_HASH_MISMATCH",
                    source_of_truth(ledger_path),
                    format!("ledger_records[{}].payload.record_refs[{index}]", packet.sequence),
                    format!(
                        "evidence reference for sequence {sequence} does not match physical ledger row"
                    ),
                    "treat the evidence packet as invalid and rebuild from an intact ledger",
                ));
            }
            checked_refs += 1;
        }
    }
    Ok(json!({
        "status": "passed",
        "checked_packets": checked_packets,
        "checked_record_refs": checked_refs,
    }))
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
             VALUES ('schema_version', '4');
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

fn run_git_with_stdin(repo: &Path, args: &[&str], stdin_text: &str) -> BbxResult<ProcessReadback> {
    let command = std::iter::once("git".to_string())
        .chain(std::iter::once("-C".to_string()))
        .chain(std::iter::once(repo.display().to_string()))
        .chain(args.iter().map(|arg| (*arg).to_string()))
        .collect::<Vec<_>>();
    let mut child = Command::new(&command[0])
        .args(&command[1..])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| {
            BlackboxError::connector_blocked(
                "GIT_EXEC_FAILED",
                GIT_CONNECTOR_ID,
                git_source_of_truth(repo, "repository"),
                "git.exec",
                format!("failed to execute git argv {:?}: {err}", command),
                "install Git and ensure the git executable is available on PATH",
            )
        })?;
    {
        let Some(mut stdin) = child.stdin.take() else {
            return Err(BlackboxError::connector_blocked(
                "GIT_STDIN_UNAVAILABLE",
                GIT_CONNECTOR_ID,
                git_source_of_truth(repo, "repository"),
                "git.stdin",
                "failed to open git stdin for patch input",
                "inspect the local process environment and retry",
            ));
        };
        stdin.write_all(stdin_text.as_bytes()).map_err(|err| {
            BlackboxError::connector_blocked(
                "GIT_STDIN_WRITE_FAILED",
                GIT_CONNECTOR_ID,
                git_source_of_truth(repo, "repository"),
                "git.stdin",
                format!("failed to write patch bytes to git stdin: {err}"),
                "inspect local process pipes and retry",
            )
        })?;
    }
    let output = child.wait_with_output().map_err(|err| {
        BlackboxError::connector_blocked(
            "GIT_EXEC_FAILED",
            GIT_CONNECTOR_ID,
            git_source_of_truth(repo, "repository"),
            "git.exec",
            format!("failed to wait for git argv {:?}: {err}", command),
            "inspect local process execution and retry",
        )
    })?;
    Ok(ProcessReadback {
        exit_code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
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
