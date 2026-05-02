//! BlackBoxFix Phase 1 local ledger and CLI.
//!
//! Source of truth: SQLite table `ledger_records` in the user-provided
//! ledger database. Every mutating command writes only after redaction,
//! schema validation, and hash-chain verification have succeeded.

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use chrono::Utc;
use clap::{Args, Parser, Subcommand, ValueEnum};
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::Serialize;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use tracing::info;
use uuid::Uuid;

const LEDGER_SCHEMA_VERSION: i64 = 1;
const REDACTION_POLICY_ID: &str = "redact_v1";
const SQLITE_CONNECTOR_ID: &str = "sqlite_local_v1";

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
    /// Record agent runs and tool calls.
    Record {
        #[command(subcommand)]
        action: RecordCommands,
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
    HashChain,
}

#[derive(Debug)]
pub struct BlackboxError {
    kind: ErrorKind,
    error_code: &'static str,
    message: String,
    source_of_truth: Option<String>,
    field_path: Option<String>,
    remediation: &'static str,
}

impl BlackboxError {
    fn new(
        kind: ErrorKind,
        error_code: &'static str,
        message: impl Into<String>,
        source_of_truth: Option<String>,
        field_path: Option<String>,
        remediation: &'static str,
    ) -> Self {
        Self {
            kind,
            error_code,
            message: message.into(),
            source_of_truth,
            field_path,
            remediation,
        }
    }

    fn invalid(message: impl Into<String>, remediation: &'static str) -> Self {
        Self::new(
            ErrorKind::InvalidInput,
            "BBX_INVALID_INPUT",
            message,
            None,
            None,
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
        Self::new(
            ErrorKind::Schema,
            "BBX_SCHEMA_VALIDATION_FAILED",
            message,
            Some(source_of_truth(path)),
            None,
            "run `blackbox init --ledger PATH` against a new ledger or migrate the existing database explicitly",
        )
    }

    fn ledger_missing(path: &Path) -> Self {
        Self::new(
            ErrorKind::LedgerMissing,
            "BBX_LEDGER_NOT_INITIALIZED",
            format!("ledger is not initialized at {}", path.display()),
            Some(source_of_truth(path)),
            None,
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
            None,
            "record the parent run first and retry with an existing run_id",
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
            "connector": SQLITE_CONNECTOR_ID,
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
    match run_blackbox_command(command) {
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

fn run_blackbox_command(command: BlackboxCommands) -> BbxResult<Value> {
    match command {
        BlackboxCommands::Init(args) => init_ledger(args),
        BlackboxCommands::Record { action } => match action {
            RecordCommands::Run(args) => record_run(args),
            RecordCommands::ToolCall(args) => record_tool_call(args),
        },
        BlackboxCommands::Inspect { action } => match action {
            InspectCommands::Ledger(args) => inspect_ledger(args),
        },
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

    let mut conn = open_connection(&args.ledger)?;
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
    }))
}

fn record_run(args: RecordRunArgs) -> BbxResult<Value> {
    require_non_empty("--task", &args.task)?;
    require_non_empty("--agent", &args.agent)?;
    require_non_empty("--tenant", &args.tenant)?;

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
    require_non_empty("--run-id", &args.run_id)?;
    require_non_empty("--tool", &args.tool)?;
    require_non_empty("--tenant", &args.tenant)?;
    require_non_empty("--agent", &args.agent)?;

    let mut conn = open_initialized_ledger(&args.ledger)?;
    ensure_run_exists(&conn, &args.ledger, &args.run_id)?;

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

fn require_non_empty(field: &'static str, value: &str) -> BbxResult<()> {
    if value.trim().is_empty() {
        return Err(BlackboxError::invalid(
            format!("{field} must not be empty"),
            "provide a non-empty value and retry",
        ));
    }
    Ok(())
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
             VALUES ('schema_version', '1');
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
    let version: Option<String> = conn
        .query_row(
            "SELECT value FROM blackbox_metadata WHERE key = 'schema_version'",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(|err| BlackboxError::sqlite(path, "read ledger schema metadata", err))?;
    match version.as_deref() {
        Some("1") => {}
        Some(other) => {
            return Err(BlackboxError::schema(
                path,
                format!("unsupported ledger schema_version {other}; expected 1"),
            ));
        }
        None => return Err(BlackboxError::ledger_missing(path)),
    }

    let table_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master
             WHERE type = 'table' AND name IN ('blackbox_metadata', 'ledger_records')",
            [],
            |row| row.get(0),
        )
        .map_err(|err| BlackboxError::sqlite(path, "inspect ledger tables", err))?;
    if table_count != 2 {
        return Err(BlackboxError::schema(
            path,
            "required tables blackbox_metadata and ledger_records are not both present",
        ));
    }

    let trigger_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master
             WHERE type = 'trigger'
               AND name IN ('ledger_records_no_update', 'ledger_records_no_delete')",
            [],
            |row| row.get(0),
        )
        .map_err(|err| BlackboxError::sqlite(path, "inspect append-only triggers", err))?;
    if trigger_count != 2 {
        return Err(BlackboxError::schema(
            path,
            "append-only triggers are missing from ledger_records",
        ));
    }

    Ok(())
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
    require_non_empty("record_kind", &record.record_kind)?;
    require_non_empty("tenant_id", &record.tenant_id)?;

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

fn ensure_run_exists(conn: &Connection, path: &Path, run_id: &str) -> BbxResult<()> {
    let exists: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM ledger_records
             WHERE record_kind = 'agent_run' AND run_id = ?1",
            params![run_id],
            |row| row.get(0),
        )
        .map_err(|err| BlackboxError::sqlite(path, "read run parent from ledger", err))?;
    if exists == 0 {
        return Err(BlackboxError::not_found(
            path,
            format!("run_id {run_id} does not exist in ledger_records"),
        ));
    }
    Ok(())
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
            redaction: RedactionReport::from_json(redaction_value.clone())?,
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

    fn from_json(value: Value) -> BbxResult<Self> {
        let policy = value
            .get("policy_id")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                BlackboxError::invalid(
                    "redaction policy_id missing",
                    "repair the ledger row or restore from backup",
                )
            })?;
        if policy != REDACTION_POLICY_ID {
            return Err(BlackboxError::invalid(
                format!("unsupported redaction policy_id {policy}"),
                "migrate the ledger row to the current redaction policy",
            ));
        }

        let redacted_fields = value
            .get("redacted_fields")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                BlackboxError::invalid(
                    "redacted_fields missing",
                    "repair the ledger row or restore from backup",
                )
            })?
            .iter()
            .map(|item| {
                item.as_str().map(ToOwned::to_owned).ok_or_else(|| {
                    BlackboxError::invalid(
                        "redacted_fields must contain strings",
                        "repair the ledger row or restore from backup",
                    )
                })
            })
            .collect::<BbxResult<Vec<_>>>()?;

        let secret_digest_fields = value
            .get("secret_digest_fields")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                BlackboxError::invalid(
                    "secret_digest_fields missing",
                    "repair the ledger row or restore from backup",
                )
            })?
            .iter()
            .map(|(key, val)| {
                val.as_str()
                    .map(|digest| (key.clone(), digest.to_string()))
                    .ok_or_else(|| {
                        BlackboxError::invalid(
                            "secret_digest_fields values must be strings",
                            "repair the ledger row or restore from backup",
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
        || trimmed.starts_with("ghp_")
        || trimmed.starts_with("github_pat_")
        || trimmed.starts_with("xoxb-")
        || trimmed.starts_with("sk-")
        || looks_like_aws_access_key(trimmed)
        || looks_like_jwt(trimmed)
}

fn looks_like_aws_access_key(text: &str) -> bool {
    text.len() == 20
        && text.starts_with("AKIA")
        && text
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
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
