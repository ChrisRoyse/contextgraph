//! DynamicJEPA CLI: domain-pack registry, physical inspection, and Phase 4
//! adapter/panel materialization.
//!
//! These commands intentionally stop at Phase 3 scope. Mutating commands write
//! to the real RocksDB DynamicJEPA column families, then immediately read the
//! source-of-truth rows back through separate storage inspection paths.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use clap::{Args, Subcommand};
use context_graph_core::dynamicjepa::{
    ActionId, ActionOrigin, ActionSchema, AdapterId, AdapterRunRecord, AdapterRunStatus,
    AdapterSpec, CandidateActionConfig, DatasetPolicy, DjRecordHeader, DomainPack, DomainPackId,
    DynamicJepaError, DynamicJepaRecord, DynamicJepaResult, EntitySchema, EventId, FieldKind,
    FieldSpec, FieldValue, InstrumentId, InstrumentKind, InstrumentReading, InstrumentSpec,
    InstrumentValidation, InvariantSpec, LatentPanel, Normalization, NormalizedAction,
    NormalizedOutcome, NormalizedState, ObjectiveSpec, OutcomeId, OutcomeSchema, PanelId,
    PanelSlot, PanelSlotKind, PayloadFormat, PlannerPolicy, RawDomainEvent, ReadingStatus,
    SourceKind, StateId, StateSchema, StateTransition, TimeEncoding, TimeSchema, TransitionId,
    VerificationPolicy, ADAPTER_RUN_RECORD_VERSION, DOMAIN_PACK_RECORD_VERSION,
    INSTRUMENT_READING_RECORD_VERSION, LATENT_PANEL_RECORD_VERSION,
    NORMALIZED_ACTION_RECORD_VERSION, NORMALIZED_OUTCOME_RECORD_VERSION,
    NORMALIZED_STATE_RECORD_VERSION, RAW_DOMAIN_EVENT_RECORD_VERSION,
    STATE_TRANSITION_RECORD_VERSION,
};
use context_graph_storage::dynamicjepa::column_families::{
    CF_DJ_ACTIONS, CF_DJ_ADAPTER_REGISTRY, CF_DJ_ADAPTER_RUNS, CF_DJ_AUDIT_LOG, CF_DJ_DOMAIN_PACKS,
    CF_DJ_DOMAIN_PACK_BY_NAME_VERSION, CF_DJ_INSTRUMENT_READINGS, CF_DJ_INSTRUMENT_REGISTRY,
    CF_DJ_LATENT_PANELS, CF_DJ_NORMALIZED_STATES, CF_DJ_OUTCOMES, CF_DJ_RAW_EVENTS,
    CF_DJ_TRANSITIONS, DJ_CF_NAMES,
};
use context_graph_storage::dynamicjepa::keys::{
    adapter_registry_key, instrument_registry_key, name_version_key, uuid_key,
};
use context_graph_storage::dynamicjepa::{
    get_action, get_adapter_run, get_adapter_spec, get_domain_pack, get_domain_pack_by_storage_id,
    get_instrument_spec, get_latent_panel, get_normalized_state, get_outcome, get_raw_event,
    get_transition, inspect_cf, list_adapter_runs, list_domain_packs,
    list_instrument_readings_for_event, list_latent_panels, list_transitions,
    put_adapter_run_failure_batch, put_adapter_run_success_batch, put_audit_record,
    put_domain_pack, put_panel_with_readings_batch, put_raw_event_and_adapter_run_started,
    snapshot_dj_counts,
};
use context_graph_storage::dynamicjepa::{AuditStatus, DjAuditRecord};
use context_graph_storage::teleological::{RocksDbTeleologicalStore, TeleologicalStoreConfig};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use tracing::{error, info};
use uuid::Uuid;

#[derive(Subcommand, Debug)]
pub enum DynamicJepaCommands {
    /// Register a strict TOML domain pack into DynamicJEPA RocksDB CFs.
    RegisterDomain(RegisterDomainArgs),
    /// List registered domain packs from the physical DB.
    ListDomains(ListDomainsArgs),
    /// Get one registered domain pack by id and version.
    GetDomain(GetDomainArgs),
    /// Count every DynamicJEPA column family in the physical DB.
    InspectCounts(InspectCountsArgs),
    /// Decode rows from one DynamicJEPA column family.
    InspectCf(InspectCfArgs),
    /// Ingest a JSONL fixture and run its registered adapter.
    IngestEvent(IngestEventArgs),
    /// Run the registered adapter for one already persisted raw event.
    RunAdapter(RunAdapterArgs),
    /// Materialize one latent panel or every pending panel for a domain.
    MaterializePanel(MaterializePanelArgs),
    /// Read one latent panel, optionally with its instrument readings.
    GetPanel(GetPanelArgs),
    /// List instrument readings for one event.
    ListInstrumentReadings(ListInstrumentReadingsArgs),
}

impl DynamicJepaCommands {
    fn operation(&self) -> &'static str {
        match self {
            Self::RegisterDomain(_) => "dynamicjepa_register_domain",
            Self::ListDomains(_) => "dynamicjepa_list_domains",
            Self::GetDomain(_) => "dynamicjepa_get_domain",
            Self::InspectCounts(_) => "dynamicjepa_inspect_counts",
            Self::InspectCf(_) => "dynamicjepa_inspect_cf",
            Self::IngestEvent(_) => "dynamicjepa_ingest_event",
            Self::RunAdapter(_) => "dynamicjepa_run_adapter",
            Self::MaterializePanel(_) => "dynamicjepa_materialize_panel",
            Self::GetPanel(_) => "dynamicjepa_get_panel",
            Self::ListInstrumentReadings(_) => "dynamicjepa_list_instrument_readings",
        }
    }
}

#[derive(Args, Debug)]
pub struct RegisterDomainArgs {
    /// Path to the RocksDB data directory.
    #[arg(long, env = "CONTEXT_GRAPH_STORAGE_PATH")]
    pub db: PathBuf,

    /// Strict DynamicJEPA domain-pack TOML.
    #[arg(long)]
    pub file: PathBuf,

    /// Emit one machine-readable JSON object.
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct ListDomainsArgs {
    /// Path to the RocksDB data directory.
    #[arg(long, env = "CONTEXT_GRAPH_STORAGE_PATH")]
    pub db: PathBuf,

    /// Maximum decoded domain-pack records to return.
    #[arg(long, default_value_t = 100)]
    pub limit: usize,

    /// Number of decoded domain-pack records to skip.
    #[arg(long, default_value_t = 0)]
    pub offset: usize,

    /// Emit one machine-readable JSON object.
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct GetDomainArgs {
    /// Path to the RocksDB data directory.
    #[arg(long, env = "CONTEXT_GRAPH_STORAGE_PATH")]
    pub db: PathBuf,

    /// Domain pack id, for example counter_world.
    #[arg(long)]
    pub id: String,

    /// Domain pack version. Phase 0 fixtures use 1.0.0.
    #[arg(long = "domain-version", default_value = "1.0.0")]
    pub domain_version: String,

    /// Emit one machine-readable JSON object.
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct InspectCountsArgs {
    /// Path to the RocksDB data directory.
    #[arg(long, env = "CONTEXT_GRAPH_STORAGE_PATH")]
    pub db: PathBuf,

    /// Emit one machine-readable JSON object.
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct InspectCfArgs {
    /// Path to the RocksDB data directory.
    #[arg(long, env = "CONTEXT_GRAPH_STORAGE_PATH")]
    pub db: PathBuf,

    /// DynamicJEPA CF name, for example dj_domain_packs.
    #[arg(long)]
    pub cf: String,

    /// Maximum rows to decode.
    #[arg(long, default_value_t = 20)]
    pub limit: usize,

    /// Number of rows to skip.
    #[arg(long, default_value_t = 0)]
    pub offset: usize,

    /// Emit one machine-readable JSON object.
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct IngestEventArgs {
    /// Path to the RocksDB data directory.
    #[arg(long, env = "CONTEXT_GRAPH_STORAGE_PATH")]
    pub db: PathBuf,

    /// Registered domain pack id, for example counter_world.
    #[arg(long)]
    pub domain: String,

    /// Registered adapter id. Phase 4 supports json_event.
    #[arg(long)]
    pub adapter: String,

    /// JSONL fixture file to ingest.
    #[arg(long)]
    pub file: PathBuf,

    /// Emit one machine-readable JSON object.
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct RunAdapterArgs {
    /// Path to the RocksDB data directory.
    #[arg(long, env = "CONTEXT_GRAPH_STORAGE_PATH")]
    pub db: PathBuf,

    /// Raw event id whose started adapter run should be completed.
    #[arg(long = "event-id")]
    pub event_id: String,

    /// Emit one machine-readable JSON object.
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct MaterializePanelArgs {
    /// Path to the RocksDB data directory.
    #[arg(long, env = "CONTEXT_GRAPH_STORAGE_PATH")]
    pub db: PathBuf,

    /// Materialize a single transition id.
    #[arg(long = "transition-id", conflicts_with = "all_pending")]
    pub transition_id: Option<String>,

    /// Materialize every transition without a panel for the domain.
    #[arg(long = "all-pending", conflicts_with = "transition_id")]
    pub all_pending: bool,

    /// Domain id required with --all-pending.
    #[arg(long)]
    pub domain: Option<String>,

    /// Emit one machine-readable JSON object.
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct GetPanelArgs {
    /// Path to the RocksDB data directory.
    #[arg(long, env = "CONTEXT_GRAPH_STORAGE_PATH")]
    pub db: PathBuf,

    /// Panel id to read.
    #[arg(long = "panel-id")]
    pub panel_id: String,

    /// Include instrument reading rows linked by the panel.
    #[arg(long = "include-readings")]
    pub include_readings: bool,

    /// Emit one machine-readable JSON object.
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct ListInstrumentReadingsArgs {
    /// Path to the RocksDB data directory.
    #[arg(long, env = "CONTEXT_GRAPH_STORAGE_PATH")]
    pub db: PathBuf,

    /// Event id whose readings should be read from dj_instrument_readings.
    #[arg(long = "event-id")]
    pub event_id: String,

    /// Emit one machine-readable JSON object.
    #[arg(long)]
    pub json: bool,
}

pub async fn handle_dynamicjepa_command(action: DynamicJepaCommands) -> i32 {
    let operation = action.operation();
    let outcome = run(action).await;
    match outcome {
        Ok(outcome) => {
            if let Err(err) = print_json(&outcome.value) {
                error!(
                    operation,
                    error_code = "CLI_JSON_OUTPUT",
                    error_message = %err,
                    "DynamicJEPA CLI could not serialize JSON output"
                );
                eprintln!("dynamicjepa command FAILED: JSON output serialization failed: {err}");
                return 1;
            }
            if outcome.exit_code == 0 {
                info!(
                    operation,
                    status = "ok",
                    "DynamicJEPA CLI command completed"
                );
            } else {
                error!(
                    operation,
                    status = "error",
                    "DynamicJEPA CLI command returned a structured error"
                );
            }
            outcome.exit_code
        }
        Err(err) => {
            let value = error_value(operation, None, &[], None, None, &err);
            let _ = print_json(&value);
            error!(
                operation,
                error_code = err.code(),
                error_message = %err,
                "DynamicJEPA CLI command failed before source-of-truth counts could be read"
            );
            1
        }
    }
}

struct CommandOutcome {
    value: Value,
    exit_code: i32,
}

async fn run(action: DynamicJepaCommands) -> DynamicJepaResult<CommandOutcome> {
    match action {
        DynamicJepaCommands::RegisterDomain(args) => register_domain(args),
        DynamicJepaCommands::ListDomains(args) => list_domains(args),
        DynamicJepaCommands::GetDomain(args) => get_domain(args),
        DynamicJepaCommands::InspectCounts(args) => inspect_counts(args),
        DynamicJepaCommands::InspectCf(args) => inspect_column_family(args),
        DynamicJepaCommands::IngestEvent(args) => ingest_event(args),
        DynamicJepaCommands::RunAdapter(args) => run_adapter_command(args),
        DynamicJepaCommands::MaterializePanel(args) => materialize_panel_command(args),
        DynamicJepaCommands::GetPanel(args) => get_panel_command(args),
        DynamicJepaCommands::ListInstrumentReadings(args) => list_instrument_readings_command(args),
    }
}

fn register_domain(args: RegisterDomainArgs) -> DynamicJepaResult<CommandOutcome> {
    let _json = args.json;
    let source_cfs = [
        CF_DJ_DOMAIN_PACKS,
        CF_DJ_DOMAIN_PACK_BY_NAME_VERSION,
        CF_DJ_INSTRUMENT_REGISTRY,
        CF_DJ_ADAPTER_REGISTRY,
        CF_DJ_AUDIT_LOG,
    ];
    let store = open_store(&args.db)?;
    let db = store.dynamicjepa_db();
    let db_path = canonical_existing_path(&args.db, "register-domain.db")?;
    let file_path = canonical_existing_path(&args.file, "register-domain.file")?;
    let before_counts = snapshot_dj_counts(db)?;

    let result = (|| -> DynamicJepaResult<Value> {
        let domain_pack = parse_domain_pack_file(&args.file)?;
        if get_domain_pack(db, &domain_pack.id, &domain_pack.version)?.is_some() {
            return Err(DynamicJepaError::validation(
                "register-domain.id_version",
                format!(
                    "domain pack {} version {} already exists",
                    domain_pack.id, domain_pack.version
                ),
                "bump the domain pack version or wipe the pre-production demo DB before registering again",
            ));
        }

        let domain_pack_uuid = put_domain_pack(db, &domain_pack)?;
        let primary = get_domain_pack_by_storage_id(db, domain_pack_uuid)?.ok_or_else(|| {
            DynamicJepaError::SourceOfTruthMissing {
                cf: CF_DJ_DOMAIN_PACKS.to_string(),
                key: uuid_key(domain_pack_uuid).to_vec(),
            }
        })?;
        let by_name_version = get_domain_pack(db, &domain_pack.id, &domain_pack.version)?
            .ok_or_else(|| DynamicJepaError::SourceOfTruthMissing {
                cf: CF_DJ_DOMAIN_PACK_BY_NAME_VERSION.to_string(),
                key: name_version_key(&domain_pack.id, &domain_pack.version).to_vec(),
            })?;

        if primary != domain_pack || by_name_version != domain_pack {
            return Err(DynamicJepaError::StorageInvariantViolation {
                message: format!(
                    "registered domain pack readback mismatch for {}:{}",
                    domain_pack.id, domain_pack.version
                ),
            });
        }

        let mut instrument_records = Vec::with_capacity(domain_pack.instrument_specs.len());
        for spec in &domain_pack.instrument_specs {
            let stored = get_instrument_spec(db, domain_pack_uuid, &spec.instrument_id)?
                .ok_or_else(|| DynamicJepaError::SourceOfTruthMissing {
                    cf: CF_DJ_INSTRUMENT_REGISTRY.to_string(),
                    key: instrument_registry_key(domain_pack_uuid, &spec.instrument_id).to_vec(),
                })?;
            if stored != *spec {
                return Err(DynamicJepaError::StorageInvariantViolation {
                    message: format!(
                        "instrument registry readback mismatch for {} in domain {}",
                        spec.instrument_id, domain_pack.id
                    ),
                });
            }
            instrument_records.push(serde_json::to_value(&stored).map_err(|err| {
                DynamicJepaError::validation(
                    "register-domain.instrument_json",
                    format!("failed to serialize instrument readback: {err}"),
                    "inspect the InstrumentSpec serde implementation",
                )
            })?);
        }

        let mut adapter_records = Vec::with_capacity(domain_pack.adapter_specs.len());
        for spec in &domain_pack.adapter_specs {
            let stored =
                get_adapter_spec(db, domain_pack_uuid, &spec.adapter_id)?.ok_or_else(|| {
                    DynamicJepaError::SourceOfTruthMissing {
                        cf: CF_DJ_ADAPTER_REGISTRY.to_string(),
                        key: adapter_registry_key(domain_pack_uuid, &spec.adapter_id).to_vec(),
                    }
                })?;
            if stored != *spec {
                return Err(DynamicJepaError::StorageInvariantViolation {
                    message: format!(
                        "adapter registry readback mismatch for {} in domain {}",
                        spec.adapter_id, domain_pack.id
                    ),
                });
            }
            adapter_records.push(serde_json::to_value(&stored).map_err(|err| {
                DynamicJepaError::validation(
                    "register-domain.adapter_json",
                    format!("failed to serialize adapter readback: {err}"),
                    "inspect the AdapterSpec serde implementation",
                )
            })?);
        }

        let cf_samples = json!({
            CF_DJ_DOMAIN_PACKS: inspect_cf(db, CF_DJ_DOMAIN_PACKS, 10, 0)?,
            CF_DJ_DOMAIN_PACK_BY_NAME_VERSION: inspect_cf(db, CF_DJ_DOMAIN_PACK_BY_NAME_VERSION, 10, 0)?,
            CF_DJ_INSTRUMENT_REGISTRY: inspect_cf(db, CF_DJ_INSTRUMENT_REGISTRY, 20, 0)?,
            CF_DJ_ADAPTER_REGISTRY: inspect_cf(db, CF_DJ_ADAPTER_REGISTRY, 20, 0)?,
            CF_DJ_AUDIT_LOG: inspect_cf(db, CF_DJ_AUDIT_LOG, 20, 0)?,
        });

        let created_ids = json!({
            "domain_pack_uuid": domain_pack_uuid.to_string(),
            "domain_pack_id": domain_pack.id.to_string(),
            "domain_pack_version": domain_pack.version,
            "instrument_ids": domain_pack
                .instrument_specs
                .iter()
                .map(|spec| spec.instrument_id.to_string())
                .collect::<Vec<_>>(),
            "adapter_ids": domain_pack
                .adapter_specs
                .iter()
                .map(|spec| spec.adapter_id.to_string())
                .collect::<Vec<_>>(),
        });

        Ok(json!({
            "created_ids": created_ids,
            "decoded_records": {
                "domain_pack": primary,
                "domain_pack_by_name_version": by_name_version,
                "instrument_registry": instrument_records,
                "adapter_registry": adapter_records,
                "cf_samples": cf_samples,
                "source_hash_sha256": hex_bytes(&domain_pack.source_hash),
            }
        }))
    })();

    let after_counts = match snapshot_dj_counts(db) {
        Ok(counts) => counts,
        Err(count_err) => {
            if let Some(primary) = result.as_ref().err() {
                return Err(DynamicJepaError::StorageInvariantViolation {
                    message: format!(
                        "register-domain failed with {primary}; after-count source-of-truth inspection also failed with {count_err}"
                    ),
                });
            }
            return Err(count_err);
        }
    };
    match result {
        Ok(decoded_records) => Ok(CommandOutcome {
            value: json!({
                "operation": "dynamicjepa_register_domain",
                "status": "ok",
                "db_path": db_path,
                "input_file": file_path,
                "source_of_truth": source_cfs,
                "before_counts": before_counts,
                "after_counts": after_counts,
                "count_delta": count_delta(&before_counts, &after_counts)?,
                "created_ids": decoded_records["created_ids"].clone(),
                "decoded_records": decoded_records["decoded_records"].clone(),
            }),
            exit_code: 0,
        }),
        Err(err) => Ok(CommandOutcome {
            value: error_value(
                "dynamicjepa_register_domain",
                Some(db_path),
                &source_cfs,
                Some(before_counts),
                Some(after_counts),
                &err,
            ),
            exit_code: 1,
        }),
    }
}

fn list_domains(args: ListDomainsArgs) -> DynamicJepaResult<CommandOutcome> {
    let _json = args.json;
    if args.limit == 0 {
        return Err(DynamicJepaError::validation(
            "list-domains.limit",
            "--limit must be greater than zero",
            "request at least one row so the source-of-truth read has observable output",
        ));
    }
    let store = open_store(&args.db)?;
    let db = store.dynamicjepa_db();
    let db_path = canonical_existing_path(&args.db, "list-domains.db")?;
    let counts = snapshot_dj_counts(db)?;
    let domains = list_domain_packs(db, args.limit, args.offset)?;
    Ok(CommandOutcome {
        value: json!({
            "operation": "dynamicjepa_list_domains",
            "status": "ok",
            "db_path": db_path,
            "source_of_truth": [CF_DJ_DOMAIN_PACKS],
            "counts": counts,
            "limit": args.limit,
            "offset": args.offset,
            "decoded_records": {
                "domains": domains,
            },
        }),
        exit_code: 0,
    })
}

fn get_domain(args: GetDomainArgs) -> DynamicJepaResult<CommandOutcome> {
    let _json = args.json;
    let store = open_store(&args.db)?;
    let db = store.dynamicjepa_db();
    let db_path = canonical_existing_path(&args.db, "get-domain.db")?;
    let counts = snapshot_dj_counts(db)?;
    let id = DomainPackId::new(args.id.clone())?;
    let record = get_domain_pack(db, &id, &args.domain_version)?;
    match record {
        Some(record) => Ok(CommandOutcome {
            value: json!({
                "operation": "dynamicjepa_get_domain",
                "status": "ok",
                "db_path": db_path,
                "source_of_truth": [CF_DJ_DOMAIN_PACKS, CF_DJ_DOMAIN_PACK_BY_NAME_VERSION],
                "counts": counts,
                "decoded_records": {
                    "domain_pack": record,
                },
            }),
            exit_code: 0,
        }),
        None => Ok(CommandOutcome {
            value: json!({
                "operation": "dynamicjepa_get_domain",
                "status": "not_found",
                "db_path": db_path,
                "source_of_truth": [CF_DJ_DOMAIN_PACKS, CF_DJ_DOMAIN_PACK_BY_NAME_VERSION],
                "counts": counts,
                "queried": {
                    "id": args.id,
                    "version": args.domain_version,
                },
                "remediation": "register the domain pack with dynamicjepa register-domain before reading it",
            }),
            exit_code: 0,
        }),
    }
}

fn inspect_counts(args: InspectCountsArgs) -> DynamicJepaResult<CommandOutcome> {
    let _json = args.json;
    let store = open_store(&args.db)?;
    let db = store.dynamicjepa_db();
    let db_path = canonical_existing_path(&args.db, "inspect-counts.db")?;
    let counts = snapshot_dj_counts(db)?;
    Ok(CommandOutcome {
        value: json!({
            "operation": "dynamicjepa_inspect_counts",
            "status": "ok",
            "db_path": db_path,
            "source_of_truth": DJ_CF_NAMES,
            "counts": counts,
            "after_counts": counts,
        }),
        exit_code: 0,
    })
}

fn inspect_column_family(args: InspectCfArgs) -> DynamicJepaResult<CommandOutcome> {
    let _json = args.json;
    if args.limit == 0 {
        return Err(DynamicJepaError::validation(
            "inspect-cf.limit",
            "--limit must be greater than zero",
            "request at least one row so the source-of-truth read has observable output",
        ));
    }
    let cf_name = resolve_dynamic_cf_name(&args.cf)?;
    let store = open_store(&args.db)?;
    let db = store.dynamicjepa_db();
    let db_path = canonical_existing_path(&args.db, "inspect-cf.db")?;
    let counts = snapshot_dj_counts(db)?;
    let rows = inspect_cf(db, cf_name, args.limit, args.offset)?;
    Ok(CommandOutcome {
        value: json!({
            "operation": "dynamicjepa_inspect_cf",
            "status": "ok",
            "db_path": db_path,
            "source_of_truth": [cf_name],
            "counts": counts,
            "cf": cf_name,
            "limit": args.limit,
            "offset": args.offset,
            "decoded_records": {
                "rows": rows,
            },
        }),
        exit_code: 0,
    })
}

fn ingest_event(args: IngestEventArgs) -> DynamicJepaResult<CommandOutcome> {
    let _json = args.json;
    let source_cfs = phase4_ingest_cfs();
    let store = open_store(&args.db)?;
    let db = store.dynamicjepa_db();
    let db_path = canonical_existing_path(&args.db, "ingest-event.db")?;
    let file_path = canonical_existing_path(&args.file, "ingest-event.file")?;
    let before_counts = snapshot_dj_counts(db)?;
    let domain_id = DomainPackId::new(args.domain.clone())?;
    let adapter_id = AdapterId::new(args.adapter.clone())?;
    let mut created = IngestCreatedIds::default();

    let result = (|| -> DynamicJepaResult<Value> {
        let domain_pack = get_domain_pack(db, &domain_id, "1.0.0")?.ok_or_else(|| {
            DynamicJepaError::DomainPackNotFound {
                id: domain_id.to_string(),
            }
        })?;
        let adapter = domain_pack
            .adapter_specs
            .iter()
            .find(|spec| spec.adapter_id == adapter_id)
            .ok_or_else(|| {
                DynamicJepaError::validation(
                    "ingest-event.adapter",
                    format!(
                        "adapter {} is not registered for domain {}",
                        adapter_id, domain_pack.id
                    ),
                    "register the domain pack with the requested adapter before ingestion",
                )
            })?
            .clone();
        ingest_jsonl_fixture(
            db,
            &domain_pack,
            &adapter,
            &args.file,
            &file_path,
            &mut created,
        )?;
        verify_ingest_created_ids(db, &created)?;
        Ok(json!({
            "created_ids": created,
            "decoded_records": phase4_ingest_decoded_records(db)?,
        }))
    })();

    let after_counts = snapshot_dj_counts(db)?;
    match result {
        Ok(value) => Ok(CommandOutcome {
            value: json!({
                "operation": "dynamicjepa_ingest_event",
                "status": "ok",
                "db_path": db_path,
                "input_file": file_path,
                "source_of_truth": source_cfs,
                "before_counts": before_counts,
                "after_counts": after_counts,
                "count_delta": count_delta(&before_counts, &after_counts)?,
                "created_ids": value["created_ids"].clone(),
                "decoded_records": value["decoded_records"].clone(),
            }),
            exit_code: 0,
        }),
        Err(err) => {
            let mut value = error_value(
                "dynamicjepa_ingest_event",
                Some(db_path),
                &source_cfs,
                Some(before_counts.clone()),
                Some(after_counts.clone()),
                &err,
            );
            enrich_phase4_error_value(
                &mut value,
                &before_counts,
                &after_counts,
                json!(created),
                phase4_ingest_decoded_records(db)?,
            )?;
            Ok(CommandOutcome {
                value,
                exit_code: 1,
            })
        }
    }
}

fn run_adapter_command(args: RunAdapterArgs) -> DynamicJepaResult<CommandOutcome> {
    let _json = args.json;
    let source_cfs = phase4_ingest_cfs();
    let store = open_store(&args.db)?;
    let db = store.dynamicjepa_db();
    let db_path = canonical_existing_path(&args.db, "run-adapter.db")?;
    let before_counts = snapshot_dj_counts(db)?;
    let event_id = parse_event_id(&args.event_id, "run-adapter.event_id")?;
    let mut created = IngestCreatedIds::default();

    let result = (|| -> DynamicJepaResult<Value> {
        run_adapter_for_event(db, event_id, &mut created)?;
        verify_ingest_created_ids(db, &created)?;
        Ok(json!({
            "created_ids": created,
            "decoded_records": phase4_ingest_decoded_records(db)?,
        }))
    })();

    let after_counts = snapshot_dj_counts(db)?;
    match result {
        Ok(value) => Ok(CommandOutcome {
            value: json!({
                "operation": "dynamicjepa_run_adapter",
                "status": "ok",
                "db_path": db_path,
                "source_of_truth": source_cfs,
                "before_counts": before_counts,
                "after_counts": after_counts,
                "count_delta": count_delta(&before_counts, &after_counts)?,
                "created_ids": value["created_ids"].clone(),
                "decoded_records": value["decoded_records"].clone(),
            }),
            exit_code: 0,
        }),
        Err(err) => {
            let mut value = error_value(
                "dynamicjepa_run_adapter",
                Some(db_path),
                &source_cfs,
                Some(before_counts.clone()),
                Some(after_counts.clone()),
                &err,
            );
            enrich_phase4_error_value(
                &mut value,
                &before_counts,
                &after_counts,
                json!(created),
                phase4_ingest_decoded_records(db)?,
            )?;
            Ok(CommandOutcome {
                value,
                exit_code: 1,
            })
        }
    }
}

fn materialize_panel_command(args: MaterializePanelArgs) -> DynamicJepaResult<CommandOutcome> {
    let _json = args.json;
    let source_cfs = [
        CF_DJ_TRANSITIONS,
        CF_DJ_NORMALIZED_STATES,
        CF_DJ_ACTIONS,
        CF_DJ_OUTCOMES,
        CF_DJ_INSTRUMENT_READINGS,
        CF_DJ_LATENT_PANELS,
        CF_DJ_AUDIT_LOG,
    ];
    let store = open_store(&args.db)?;
    let db = store.dynamicjepa_db();
    let db_path = canonical_existing_path(&args.db, "materialize-panel.db")?;
    let before_counts = snapshot_dj_counts(db)?;
    let mut created = PanelCreatedIds::default();

    let result = (|| -> DynamicJepaResult<Value> {
        if args.all_pending {
            let domain_raw = args.domain.as_ref().ok_or_else(|| {
                DynamicJepaError::validation(
                    "materialize-panel.domain",
                    "--domain is required with --all-pending",
                    "provide the registered domain id whose pending transitions should be scanned",
                )
            })?;
            let domain_id = DomainPackId::new(domain_raw.clone())?;
            let domain_pack = get_domain_pack(db, &domain_id, "1.0.0")?.ok_or_else(|| {
                DynamicJepaError::DomainPackNotFound {
                    id: domain_id.to_string(),
                }
            })?;
            let transitions = list_transitions(db, 1_000_000, 0)?;
            let mut skipped = 0u64;
            for transition in transitions
                .into_iter()
                .filter(|transition| transition.header.domain_pack_id == domain_pack.id)
            {
                if panel_for_transition(db, &transition)?.is_some() {
                    skipped += 1;
                    continue;
                }
                let evidence = materialize_one_panel(db, &domain_pack, transition)?;
                created.push(evidence);
            }
            Ok(json!({
                "created_ids": created,
                "summary": {
                    "materialized": created.panel_ids.len(),
                    "skipped_existing": skipped,
                },
                "decoded_records": phase4_panel_decoded_records(db)?,
            }))
        } else {
            let transition_raw = args.transition_id.as_ref().ok_or_else(|| {
                DynamicJepaError::validation(
                    "materialize-panel.transition_id",
                    "either --transition-id or --all-pending must be provided",
                    "choose one materialization mode explicitly",
                )
            })?;
            let transition_id =
                parse_transition_id(transition_raw, "materialize-panel.transition_id")?;
            let transition = get_transition(db, transition_id)?.ok_or_else(|| {
                DynamicJepaError::SourceOfTruthMissing {
                    cf: CF_DJ_TRANSITIONS.to_string(),
                    key: transition_id.into_bytes().to_vec(),
                }
            })?;
            if panel_for_transition(db, &transition)?.is_some() {
                return Err(DynamicJepaError::validation(
                    "materialize-panel.transition_id",
                    format!("transition {transition_id} already has a latent panel"),
                    "use --all-pending to skip existing panels or wipe the demo DB before rebuilding",
                ));
            }
            let domain_pack = get_domain_pack(
                db,
                &transition.header.domain_pack_id,
                &transition.header.domain_pack_version,
            )?
            .ok_or_else(|| DynamicJepaError::DomainPackNotFound {
                id: transition.header.domain_pack_id.to_string(),
            })?;
            let evidence = materialize_one_panel(db, &domain_pack, transition)?;
            created.push(evidence);
            Ok(json!({
                "created_ids": created,
                "summary": {
                    "materialized": created.panel_ids.len(),
                    "skipped_existing": 0,
                },
                "decoded_records": phase4_panel_decoded_records(db)?,
            }))
        }
    })();

    let after_counts = snapshot_dj_counts(db)?;
    match result {
        Ok(value) => Ok(CommandOutcome {
            value: json!({
                "operation": "dynamicjepa_materialize_panel",
                "status": "ok",
                "db_path": db_path,
                "source_of_truth": source_cfs,
                "before_counts": before_counts,
                "after_counts": after_counts,
                "count_delta": count_delta(&before_counts, &after_counts)?,
                "created_ids": value["created_ids"].clone(),
                "summary": value["summary"].clone(),
                "decoded_records": value["decoded_records"].clone(),
            }),
            exit_code: 0,
        }),
        Err(err) => {
            let mut value = error_value(
                "dynamicjepa_materialize_panel",
                Some(db_path),
                &source_cfs,
                Some(before_counts.clone()),
                Some(after_counts.clone()),
                &err,
            );
            enrich_phase4_error_value(
                &mut value,
                &before_counts,
                &after_counts,
                json!(created),
                phase4_panel_decoded_records(db)?,
            )?;
            Ok(CommandOutcome {
                value,
                exit_code: 1,
            })
        }
    }
}

fn get_panel_command(args: GetPanelArgs) -> DynamicJepaResult<CommandOutcome> {
    let _json = args.json;
    let store = open_store(&args.db)?;
    let db = store.dynamicjepa_db();
    let db_path = canonical_existing_path(&args.db, "get-panel.db")?;
    let counts = snapshot_dj_counts(db)?;
    let panel_id = parse_panel_id(&args.panel_id, "get-panel.panel_id")?;
    let panel = get_latent_panel(db, panel_id)?;
    match panel {
        Some(panel) => {
            let readings = if args.include_readings {
                list_instrument_readings_for_event(db, panel.event_id)?
            } else {
                Vec::new()
            };
            Ok(CommandOutcome {
                value: json!({
                    "operation": "dynamicjepa_get_panel",
                    "status": "ok",
                    "db_path": db_path,
                    "source_of_truth": [CF_DJ_LATENT_PANELS, CF_DJ_INSTRUMENT_READINGS],
                    "counts": counts,
                    "decoded_records": {
                        "panel": panel,
                        "instrument_readings": readings,
                    },
                }),
                exit_code: 0,
            })
        }
        None => Ok(CommandOutcome {
            value: json!({
                "operation": "dynamicjepa_get_panel",
                "status": "not_found",
                "db_path": db_path,
                "source_of_truth": [CF_DJ_LATENT_PANELS],
                "counts": counts,
                "queried": {"panel_id": args.panel_id},
                "remediation": "materialize the panel from a persisted transition before reading it",
            }),
            exit_code: 0,
        }),
    }
}

fn list_instrument_readings_command(
    args: ListInstrumentReadingsArgs,
) -> DynamicJepaResult<CommandOutcome> {
    let _json = args.json;
    let store = open_store(&args.db)?;
    let db = store.dynamicjepa_db();
    let db_path = canonical_existing_path(&args.db, "list-instrument-readings.db")?;
    let counts = snapshot_dj_counts(db)?;
    let event_id = parse_event_id(&args.event_id, "list-instrument-readings.event_id")?;
    let readings = list_instrument_readings_for_event(db, event_id)?;
    Ok(CommandOutcome {
        value: json!({
            "operation": "dynamicjepa_list_instrument_readings",
            "status": "ok",
            "db_path": db_path,
            "source_of_truth": [CF_DJ_INSTRUMENT_READINGS],
            "counts": counts,
            "queried": {"event_id": args.event_id},
            "decoded_records": {
                "instrument_readings": readings,
            },
        }),
        exit_code: 0,
    })
}

#[derive(Debug, Default, Clone, Serialize)]
struct IngestCreatedIds {
    raw_event_ids: Vec<String>,
    adapter_run_ids: Vec<String>,
    state_ids: Vec<String>,
    action_ids: Vec<String>,
    outcome_ids: Vec<String>,
    transition_ids: Vec<String>,
    failed_adapter_run_ids: Vec<String>,
}

#[derive(Debug, Default, Clone, Serialize)]
struct PanelCreatedIds {
    panel_ids: Vec<String>,
    event_ids: Vec<String>,
    transition_ids: Vec<String>,
    instrument_reading_ids: Vec<String>,
}

impl PanelCreatedIds {
    fn push(&mut self, evidence: MaterializedPanelEvidence) {
        self.panel_ids.push(evidence.panel_id);
        self.event_ids.push(evidence.event_id);
        self.transition_ids.push(evidence.transition_id);
        self.instrument_reading_ids
            .extend(evidence.instrument_reading_ids);
    }
}

struct MaterializedPanelEvidence {
    panel_id: String,
    event_id: String,
    transition_id: String,
    instrument_reading_ids: Vec<String>,
}

#[derive(Debug)]
struct AdapterLineError {
    message: String,
    field_path: String,
    expected_kind: String,
    actual_kind: String,
}

#[derive(Debug)]
struct AdaptedLine {
    state_fields: BTreeMap<String, FieldValue>,
    action_fields: BTreeMap<String, FieldValue>,
    outcome_fields: BTreeMap<String, FieldValue>,
    time_fields: BTreeMap<String, FieldValue>,
}

fn phase4_ingest_cfs() -> [&'static str; 7] {
    [
        CF_DJ_RAW_EVENTS,
        CF_DJ_ADAPTER_RUNS,
        CF_DJ_NORMALIZED_STATES,
        CF_DJ_ACTIONS,
        CF_DJ_OUTCOMES,
        CF_DJ_TRANSITIONS,
        CF_DJ_AUDIT_LOG,
    ]
}

fn ingest_jsonl_fixture(
    db: &rocksdb::DB,
    domain_pack: &DomainPack,
    adapter: &AdapterSpec,
    source_file: &Path,
    source_uri: &str,
    created: &mut IngestCreatedIds,
) -> DynamicJepaResult<()> {
    if adapter.kind != "json_event" {
        return Err(DynamicJepaError::validation(
            "ingest-event.adapter.kind",
            format!("unsupported adapter kind {:?}", adapter.kind),
            "Phase 4 supports json_event only",
        ));
    }
    let bytes = fs::read(source_file).map_err(|err| {
        DynamicJepaError::validation(
            "ingest-event.file",
            format!(
                "failed to read JSONL fixture {}: {err}",
                source_file.display()
            ),
            "provide an existing JSONL fixture file",
        )
    })?;
    if bytes.is_empty() {
        let err = DynamicJepaError::AdapterFailed {
            adapter_id: adapter.adapter_id.to_string(),
            event_id: Uuid::nil(),
            message: "empty input file".to_string(),
            field_path: String::new(),
        };
        let audit = build_audit(
            "ingest_event",
            AuditStatus::Failed {
                error_code: err.code().to_string(),
            },
            vec![source_uri.to_string()],
            Vec::new(),
            vec![CF_DJ_AUDIT_LOG.to_string()],
            Vec::new(),
        )?;
        put_audit_record(db, &audit)?;
        return Err(err);
    }

    let run_id = Uuid::new_v4();
    let mut offset = 0u64;
    for line in bytes.split_inclusive(|byte| *byte == b'\n') {
        let line_offset = offset;
        offset += line.len() as u64;
        process_fixture_line(
            db,
            domain_pack,
            adapter,
            source_uri,
            line_offset,
            line,
            run_id,
            created,
        )?;
    }
    Ok(())
}

fn process_fixture_line(
    db: &rocksdb::DB,
    domain_pack: &DomainPack,
    adapter: &AdapterSpec,
    source_uri: &str,
    source_offset: u64,
    line_bytes: &[u8],
    run_id: Uuid,
    created: &mut IngestCreatedIds,
) -> DynamicJepaResult<()> {
    let now = now_unix_ms()?;
    let event_id = EventId::new_v4();
    let adapter_run_id = Uuid::new_v4();
    let mut raw_event = RawDomainEvent {
        header: DjRecordHeader::new(
            event_id.0,
            RAW_DOMAIN_EVENT_RECORD_VERSION,
            domain_pack.id.clone(),
            domain_pack.version.clone(),
            now,
            None,
        ),
        event_id,
        domain_pack_id: domain_pack.id.clone(),
        adapter_id: adapter.adapter_id.clone(),
        source_kind: SourceKind::JsonlFixture,
        source_uri: source_uri.to_string(),
        source_offset,
        payload_format: PayloadFormat::Json,
        payload_bytes: line_bytes.to_vec(),
        payload_hash: sha256(line_bytes),
        received_at_unix_ms: now,
    };
    raw_event.refresh_content_hash()?;
    let mut started_run = AdapterRunRecord {
        header: DjRecordHeader::new(
            adapter_run_id,
            ADAPTER_RUN_RECORD_VERSION,
            domain_pack.id.clone(),
            domain_pack.version.clone(),
            now,
            Some(run_id),
        ),
        adapter_run_id,
        adapter_id: adapter.adapter_id.clone(),
        domain_pack_id: domain_pack.id.clone(),
        event_id,
        started_at_unix_ms: now,
        finished_at_unix_ms: None,
        status: AdapterRunStatus::Started,
        error_code: None,
        error_message: None,
        field_path: None,
        expected_kind: None,
        actual_kind: None,
        output_state_id: None,
        output_action_id: None,
        output_outcome_id: None,
        output_transition_id: None,
    };
    started_run.refresh_content_hash()?;
    let ingest_audit = build_audit(
        "ingest_raw_event",
        AuditStatus::Ok,
        vec![source_uri.to_string(), source_offset.to_string()],
        vec![event_id.to_string(), adapter_run_id.to_string()],
        vec![
            CF_DJ_RAW_EVENTS.to_string(),
            CF_DJ_ADAPTER_RUNS.to_string(),
            CF_DJ_AUDIT_LOG.to_string(),
        ],
        vec![
            raw_event.header.content_hash,
            started_run.header.content_hash,
        ],
    )?;
    put_raw_event_and_adapter_run_started(db, &raw_event, &started_run, &ingest_audit)?;
    created.raw_event_ids.push(event_id.to_string());
    created.adapter_run_ids.push(adapter_run_id.to_string());

    match adapt_payload(domain_pack, adapter, line_bytes) {
        Ok(adapted) => {
            let state_id = StateId::new_v4();
            let action_id = ActionId::new_v4();
            let outcome_id = OutcomeId::new_v4();
            let transition_id = TransitionId::new_v4();
            let timestamp_ms = adapted_timestamp_ms(domain_pack, &adapted)?;
            let mut state = NormalizedState {
                header: DjRecordHeader::new(
                    state_id.0,
                    NORMALIZED_STATE_RECORD_VERSION,
                    domain_pack.id.clone(),
                    domain_pack.version.clone(),
                    now_unix_ms()?,
                    Some(run_id),
                ),
                state_id,
                fields: adapted.state_fields,
                source_event_id: event_id,
            };
            state.refresh_content_hash()?;
            let mut action = NormalizedAction {
                header: DjRecordHeader::new(
                    action_id.0,
                    NORMALIZED_ACTION_RECORD_VERSION,
                    domain_pack.id.clone(),
                    domain_pack.version.clone(),
                    now_unix_ms()?,
                    Some(run_id),
                ),
                action_id,
                fields: adapted.action_fields,
                source_event_id: event_id,
                action_origin: ActionOrigin::Observed,
            };
            action.refresh_content_hash()?;
            let mut outcome = NormalizedOutcome {
                header: DjRecordHeader::new(
                    outcome_id.0,
                    NORMALIZED_OUTCOME_RECORD_VERSION,
                    domain_pack.id.clone(),
                    domain_pack.version.clone(),
                    now_unix_ms()?,
                    Some(run_id),
                ),
                outcome_id,
                fields: adapted.outcome_fields,
                source_event_id: event_id,
            };
            outcome.refresh_content_hash()?;
            let mut transition = StateTransition {
                header: DjRecordHeader::new(
                    transition_id.0,
                    STATE_TRANSITION_RECORD_VERSION,
                    domain_pack.id.clone(),
                    domain_pack.version.clone(),
                    now_unix_ms()?,
                    Some(run_id),
                ),
                transition_id,
                prior_state: state_id,
                action: action_id,
                outcome: outcome_id,
                next_state: state_id,
                timestamp_ms,
            };
            transition.refresh_content_hash()?;
            let mut completed_run = started_run.clone();
            completed_run.finished_at_unix_ms = Some(now_unix_ms()?);
            completed_run.status = AdapterRunStatus::Completed;
            completed_run.output_state_id = Some(state_id);
            completed_run.output_action_id = Some(action_id);
            completed_run.output_outcome_id = Some(outcome_id);
            completed_run.output_transition_id = Some(transition_id);
            completed_run.refresh_content_hash()?;
            let audit = build_audit(
                "complete_adapter_run",
                AuditStatus::Ok,
                vec![event_id.to_string(), adapter_run_id.to_string()],
                vec![
                    state_id.to_string(),
                    action_id.to_string(),
                    outcome_id.to_string(),
                    transition_id.to_string(),
                ],
                vec![
                    CF_DJ_ADAPTER_RUNS.to_string(),
                    CF_DJ_NORMALIZED_STATES.to_string(),
                    CF_DJ_ACTIONS.to_string(),
                    CF_DJ_OUTCOMES.to_string(),
                    CF_DJ_TRANSITIONS.to_string(),
                    CF_DJ_AUDIT_LOG.to_string(),
                ],
                vec![
                    completed_run.header.content_hash,
                    state.header.content_hash,
                    action.header.content_hash,
                    outcome.header.content_hash,
                    transition.header.content_hash,
                ],
            )?;
            put_adapter_run_success_batch(
                db,
                &completed_run,
                &state,
                &action,
                &outcome,
                &transition,
                &audit,
            )?;
            created.state_ids.push(state_id.to_string());
            created.action_ids.push(action_id.to_string());
            created.outcome_ids.push(outcome_id.to_string());
            created.transition_ids.push(transition_id.to_string());
            Ok(())
        }
        Err(line_err) => {
            let mut failed_run = started_run;
            failed_run.finished_at_unix_ms = Some(now_unix_ms()?);
            failed_run.status = AdapterRunStatus::Failed;
            failed_run.error_code = Some("ADAPTER_FAILED".to_string());
            failed_run.error_message = Some(line_err.message.clone());
            failed_run.field_path = Some(line_err.field_path.clone());
            failed_run.expected_kind = Some(line_err.expected_kind.clone());
            failed_run.actual_kind = Some(line_err.actual_kind.clone());
            failed_run.refresh_content_hash()?;
            let audit = build_audit(
                "complete_adapter_run",
                AuditStatus::Failed {
                    error_code: "ADAPTER_FAILED".to_string(),
                },
                vec![event_id.to_string(), adapter_run_id.to_string()],
                vec![adapter_run_id.to_string()],
                vec![CF_DJ_ADAPTER_RUNS.to_string(), CF_DJ_AUDIT_LOG.to_string()],
                vec![failed_run.header.content_hash],
            )?;
            put_adapter_run_failure_batch(db, &failed_run, &audit)?;
            created
                .failed_adapter_run_ids
                .push(adapter_run_id.to_string());
            Err(DynamicJepaError::AdapterFailed {
                adapter_id: adapter.adapter_id.to_string(),
                event_id: event_id.0,
                message: line_err.message,
                field_path: line_err.field_path,
            })
        }
    }
}

fn run_adapter_for_event(
    db: &rocksdb::DB,
    event_id: EventId,
    created: &mut IngestCreatedIds,
) -> DynamicJepaResult<()> {
    let raw_event =
        get_raw_event(db, event_id)?.ok_or_else(|| DynamicJepaError::SourceOfTruthMissing {
            cf: CF_DJ_RAW_EVENTS.to_string(),
            key: event_id.into_bytes().to_vec(),
        })?;
    let domain_pack = get_domain_pack(
        db,
        &raw_event.header.domain_pack_id,
        &raw_event.header.domain_pack_version,
    )?
    .ok_or_else(|| DynamicJepaError::DomainPackNotFound {
        id: raw_event.header.domain_pack_id.to_string(),
    })?;
    let adapter = domain_pack
        .adapter_specs
        .iter()
        .find(|spec| spec.adapter_id == raw_event.adapter_id)
        .ok_or_else(|| {
            DynamicJepaError::validation(
                "run-adapter.adapter",
                format!(
                    "adapter {} is not registered for domain {}",
                    raw_event.adapter_id, domain_pack.id
                ),
                "inspect the domain pack adapter registry before running the adapter",
            )
        })?
        .clone();
    let runs = list_adapter_runs(db, 1_000_000, 0)?;
    let matching: Vec<_> = runs
        .into_iter()
        .filter(|run| run.event_id == event_id)
        .collect();
    if matching.len() != 1 {
        return Err(DynamicJepaError::StorageInvariantViolation {
            message: format!(
                "expected exactly one adapter run for event {event_id}, found {}",
                matching.len()
            ),
        });
    }
    let started_run = matching.into_iter().next().expect("len checked");
    if started_run.status != AdapterRunStatus::Started {
        return Err(DynamicJepaError::validation(
            "run-adapter.event_id",
            format!(
                "adapter run {} for event {} is already {:?}",
                started_run.adapter_run_id, event_id, started_run.status
            ),
            "run-adapter only completes a persisted Started row; use inspect-cf to read terminal rows",
        ));
    }
    let run_id = started_run
        .header
        .source_run_id
        .unwrap_or_else(Uuid::new_v4);
    let source_uri = raw_event.source_uri.clone();
    process_existing_raw_event(
        db,
        &domain_pack,
        &adapter,
        &raw_event,
        started_run,
        run_id,
        &source_uri,
        created,
    )
}

fn process_existing_raw_event(
    db: &rocksdb::DB,
    domain_pack: &DomainPack,
    adapter: &AdapterSpec,
    raw_event: &RawDomainEvent,
    started_run: AdapterRunRecord,
    run_id: Uuid,
    _source_uri: &str,
    created: &mut IngestCreatedIds,
) -> DynamicJepaResult<()> {
    match adapt_payload(domain_pack, adapter, &raw_event.payload_bytes) {
        Ok(adapted) => {
            let state_id = StateId::new_v4();
            let action_id = ActionId::new_v4();
            let outcome_id = OutcomeId::new_v4();
            let transition_id = TransitionId::new_v4();
            let timestamp_ms = adapted_timestamp_ms(domain_pack, &adapted)?;
            let mut state = NormalizedState {
                header: DjRecordHeader::new(
                    state_id.0,
                    NORMALIZED_STATE_RECORD_VERSION,
                    domain_pack.id.clone(),
                    domain_pack.version.clone(),
                    now_unix_ms()?,
                    Some(run_id),
                ),
                state_id,
                fields: adapted.state_fields,
                source_event_id: raw_event.event_id,
            };
            state.refresh_content_hash()?;
            let mut action = NormalizedAction {
                header: DjRecordHeader::new(
                    action_id.0,
                    NORMALIZED_ACTION_RECORD_VERSION,
                    domain_pack.id.clone(),
                    domain_pack.version.clone(),
                    now_unix_ms()?,
                    Some(run_id),
                ),
                action_id,
                fields: adapted.action_fields,
                source_event_id: raw_event.event_id,
                action_origin: ActionOrigin::Observed,
            };
            action.refresh_content_hash()?;
            let mut outcome = NormalizedOutcome {
                header: DjRecordHeader::new(
                    outcome_id.0,
                    NORMALIZED_OUTCOME_RECORD_VERSION,
                    domain_pack.id.clone(),
                    domain_pack.version.clone(),
                    now_unix_ms()?,
                    Some(run_id),
                ),
                outcome_id,
                fields: adapted.outcome_fields,
                source_event_id: raw_event.event_id,
            };
            outcome.refresh_content_hash()?;
            let mut transition = StateTransition {
                header: DjRecordHeader::new(
                    transition_id.0,
                    STATE_TRANSITION_RECORD_VERSION,
                    domain_pack.id.clone(),
                    domain_pack.version.clone(),
                    now_unix_ms()?,
                    Some(run_id),
                ),
                transition_id,
                prior_state: state_id,
                action: action_id,
                outcome: outcome_id,
                next_state: state_id,
                timestamp_ms,
            };
            transition.refresh_content_hash()?;
            let mut completed_run = started_run;
            completed_run.finished_at_unix_ms = Some(now_unix_ms()?);
            completed_run.status = AdapterRunStatus::Completed;
            completed_run.output_state_id = Some(state_id);
            completed_run.output_action_id = Some(action_id);
            completed_run.output_outcome_id = Some(outcome_id);
            completed_run.output_transition_id = Some(transition_id);
            completed_run.refresh_content_hash()?;
            let audit = build_audit(
                "complete_adapter_run",
                AuditStatus::Ok,
                vec![raw_event.event_id.to_string()],
                vec![
                    state_id.to_string(),
                    action_id.to_string(),
                    outcome_id.to_string(),
                    transition_id.to_string(),
                ],
                vec![
                    CF_DJ_ADAPTER_RUNS.to_string(),
                    CF_DJ_NORMALIZED_STATES.to_string(),
                    CF_DJ_ACTIONS.to_string(),
                    CF_DJ_OUTCOMES.to_string(),
                    CF_DJ_TRANSITIONS.to_string(),
                    CF_DJ_AUDIT_LOG.to_string(),
                ],
                vec![
                    completed_run.header.content_hash,
                    state.header.content_hash,
                    action.header.content_hash,
                    outcome.header.content_hash,
                    transition.header.content_hash,
                ],
            )?;
            put_adapter_run_success_batch(
                db,
                &completed_run,
                &state,
                &action,
                &outcome,
                &transition,
                &audit,
            )?;
            created.state_ids.push(state_id.to_string());
            created.action_ids.push(action_id.to_string());
            created.outcome_ids.push(outcome_id.to_string());
            created.transition_ids.push(transition_id.to_string());
            Ok(())
        }
        Err(line_err) => {
            let mut failed_run = started_run;
            failed_run.finished_at_unix_ms = Some(now_unix_ms()?);
            failed_run.status = AdapterRunStatus::Failed;
            failed_run.error_code = Some("ADAPTER_FAILED".to_string());
            failed_run.error_message = Some(line_err.message.clone());
            failed_run.field_path = Some(line_err.field_path.clone());
            failed_run.expected_kind = Some(line_err.expected_kind.clone());
            failed_run.actual_kind = Some(line_err.actual_kind.clone());
            failed_run.refresh_content_hash()?;
            let audit = build_audit(
                "complete_adapter_run",
                AuditStatus::Failed {
                    error_code: "ADAPTER_FAILED".to_string(),
                },
                vec![raw_event.event_id.to_string()],
                vec![failed_run.adapter_run_id.to_string()],
                vec![CF_DJ_ADAPTER_RUNS.to_string(), CF_DJ_AUDIT_LOG.to_string()],
                vec![failed_run.header.content_hash],
            )?;
            put_adapter_run_failure_batch(db, &failed_run, &audit)?;
            created
                .failed_adapter_run_ids
                .push(failed_run.adapter_run_id.to_string());
            Err(DynamicJepaError::AdapterFailed {
                adapter_id: adapter.adapter_id.to_string(),
                event_id: raw_event.event_id.0,
                message: line_err.message,
                field_path: line_err.field_path,
            })
        }
    }
}

fn adapt_payload(
    domain_pack: &DomainPack,
    adapter: &AdapterSpec,
    payload_bytes: &[u8],
) -> Result<AdaptedLine, AdapterLineError> {
    let trimmed = trim_jsonl_newline(payload_bytes);
    let root: Value = serde_json::from_slice(trimmed).map_err(|err| AdapterLineError {
        message: format!("json parse failed: {err}"),
        field_path: String::new(),
        expected_kind: "valid JSON object".to_string(),
        actual_kind: "invalid_json".to_string(),
    })?;
    if !root.is_object() {
        return Err(AdapterLineError {
            message: "json_event payload must be a JSON object".to_string(),
            field_path: "$".to_string(),
            expected_kind: "object".to_string(),
            actual_kind: actual_json_kind(&root),
        });
    }
    reject_extra_json_fields(&root, adapter)?;

    let mut state_fields = BTreeMap::new();
    let mut action_fields = BTreeMap::new();
    let mut outcome_fields = BTreeMap::new();
    let mut time_fields = BTreeMap::new();

    for target_path in ordered_adapter_targets(domain_pack, adapter) {
        let source_path = adapter
            .mapping
            .get(&target_path)
            .ok_or_else(|| AdapterLineError {
                message: format!("adapter target path {target_path:?} disappeared during ordering"),
                field_path: target_path.clone(),
                expected_kind: "registered adapter mapping".to_string(),
                actual_kind: "missing".to_string(),
            })?;
        let field_spec =
            schema_field_for_path(domain_pack, &target_path).ok_or_else(|| AdapterLineError {
                message: format!("adapter target path {target_path:?} is not declared in schemas"),
                field_path: target_path.clone(),
                expected_kind: "declared schema field".to_string(),
                actual_kind: "undeclared".to_string(),
            })?;
        let value = resolve_json_path(&root, source_path).ok_or_else(|| AdapterLineError {
            message: format!("JSON path {source_path:?} did not resolve for {target_path}"),
            field_path: target_path.clone(),
            expected_kind: expected_kind_name(&field_spec.kind),
            actual_kind: "missing".to_string(),
        })?;
        let coerced = coerce_json_value(value, field_spec, &target_path)?;
        let (scope, field_name) = target_path
            .split_once('.')
            .ok_or_else(|| AdapterLineError {
                message: format!("adapter target path {target_path:?} is not dotted"),
                field_path: target_path.clone(),
                expected_kind: "scope.field".to_string(),
                actual_kind: "invalid_path".to_string(),
            })?;
        match scope {
            "state" => {
                state_fields.insert(field_name.to_string(), coerced);
            }
            "action" => {
                action_fields.insert(field_name.to_string(), coerced);
            }
            "outcome" => {
                outcome_fields.insert(field_name.to_string(), coerced);
            }
            "time" => {
                time_fields.insert(field_name.to_string(), coerced);
            }
            "entity" => {}
            _ => {
                return Err(AdapterLineError {
                    message: format!("unsupported adapter target scope {scope:?}"),
                    field_path: target_path.clone(),
                    expected_kind: "state/action/outcome/time/entity".to_string(),
                    actual_kind: scope.to_string(),
                });
            }
        }
    }

    ensure_required_schema_fields(domain_pack, "state", &state_fields)?;
    ensure_required_schema_fields(domain_pack, "action", &action_fields)?;
    ensure_required_schema_fields(domain_pack, "outcome", &outcome_fields)?;
    ensure_time_field(domain_pack, &time_fields)?;

    Ok(AdaptedLine {
        state_fields,
        action_fields,
        outcome_fields,
        time_fields,
    })
}

fn ordered_adapter_targets(domain_pack: &DomainPack, adapter: &AdapterSpec) -> Vec<String> {
    let mut ordered = Vec::new();
    for field in &domain_pack.state_schema.fields {
        let path = format!("state.{}", field.name);
        if adapter.mapping.contains_key(&path) {
            ordered.push(path);
        }
    }
    for field in &domain_pack.action_schema.fields {
        let path = format!("action.{}", field.name);
        if adapter.mapping.contains_key(&path) {
            ordered.push(path);
        }
    }
    for field in &domain_pack.outcome_schema.fields {
        let path = format!("outcome.{}", field.name);
        if adapter.mapping.contains_key(&path) {
            ordered.push(path);
        }
    }
    let time_path = format!("time.{}", domain_pack.time_schema.field.name);
    if adapter.mapping.contains_key(&time_path) {
        ordered.push(time_path);
    }
    for field in &domain_pack.entity_schema.fields {
        let path = format!("entity.{}", field.name);
        if adapter.mapping.contains_key(&path) {
            ordered.push(path);
        }
    }
    for key in adapter.mapping.keys() {
        if !ordered.iter().any(|path| path == key) {
            ordered.push(key.clone());
        }
    }
    ordered
}

fn trim_jsonl_newline(bytes: &[u8]) -> &[u8] {
    let mut end = bytes.len();
    while end > 0 && matches!(bytes[end - 1], b'\n' | b'\r') {
        end -= 1;
    }
    &bytes[..end]
}

fn reject_extra_json_fields(root: &Value, adapter: &AdapterSpec) -> Result<(), AdapterLineError> {
    let expected: BTreeSet<String> = adapter.mapping.values().cloned().collect();
    let mut observed = Vec::new();
    collect_json_leaf_paths(root, "$", &mut observed);
    for path in observed {
        if !expected.contains(&path) {
            return Err(AdapterLineError {
                message: format!("unexpected JSON field {path} is not declared in adapter mapping"),
                field_path: path,
                expected_kind: "only declared adapter mapping JSON paths".to_string(),
                actual_kind: "extra_field".to_string(),
            });
        }
    }
    Ok(())
}

fn collect_json_leaf_paths(value: &Value, path: &str, out: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            if map.is_empty() {
                out.push(path.to_string());
            }
            for (key, child) in map {
                collect_json_leaf_paths(child, &format!("{path}.{key}"), out);
            }
        }
        Value::Array(_) => out.push(path.to_string()),
        _ => out.push(path.to_string()),
    }
}

fn resolve_json_path<'a>(root: &'a Value, path: &str) -> Option<&'a Value> {
    if !path.starts_with("$.") || path.contains("[") || path.contains("..") {
        return None;
    }
    let mut current = root;
    for segment in path.trim_start_matches("$.").split('.') {
        if segment.is_empty() {
            return None;
        }
        current = current.get(segment)?;
    }
    Some(current)
}

fn schema_field_for_path<'a>(domain_pack: &'a DomainPack, path: &str) -> Option<&'a FieldSpec> {
    let (scope, name) = path.split_once('.')?;
    match scope {
        "state" => domain_pack
            .state_schema
            .fields
            .iter()
            .find(|field| field.name == name),
        "action" => domain_pack
            .action_schema
            .fields
            .iter()
            .find(|field| field.name == name),
        "outcome" => domain_pack
            .outcome_schema
            .fields
            .iter()
            .find(|field| field.name == name),
        "entity" => domain_pack
            .entity_schema
            .fields
            .iter()
            .find(|field| field.name == name),
        "time" if domain_pack.time_schema.field.name == name => {
            Some(&domain_pack.time_schema.field)
        }
        _ => None,
    }
}

fn coerce_json_value(
    value: &Value,
    field: &FieldSpec,
    field_path: &str,
) -> Result<FieldValue, AdapterLineError> {
    let coerced = match &field.kind {
        FieldKind::I64 => {
            let value = value
                .as_i64()
                .ok_or_else(|| type_error(value, field, field_path))?;
            validate_numeric_bounds(value as f64, field, field_path)?;
            FieldValue::I64(value)
        }
        FieldKind::UnixMs => {
            let value = value
                .as_i64()
                .ok_or_else(|| type_error(value, field, field_path))?;
            validate_numeric_bounds(value as f64, field, field_path)?;
            FieldValue::UnixMs(value)
        }
        FieldKind::F64 => {
            let value = value
                .as_f64()
                .ok_or_else(|| type_error(value, field, field_path))?;
            validate_numeric_bounds(value, field, field_path)?;
            FieldValue::F64(value)
        }
        FieldKind::Bool => {
            let value = value
                .as_bool()
                .ok_or_else(|| type_error(value, field, field_path))?;
            FieldValue::Bool(value)
        }
        FieldKind::String => {
            let value = value
                .as_str()
                .ok_or_else(|| type_error(value, field, field_path))?;
            FieldValue::String(value.to_string())
        }
        FieldKind::Categorical { variants } => {
            let value = value
                .as_str()
                .ok_or_else(|| type_error(value, field, field_path))?;
            if !variants.iter().any(|variant| variant == value) {
                return Err(AdapterLineError {
                    message: format!("value {value:?} is not a declared variant of {field_path}"),
                    field_path: field_path.to_string(),
                    expected_kind: expected_kind_name(&field.kind),
                    actual_kind: actual_json_kind(&Value::String(value.to_string())),
                });
            }
            FieldValue::Categorical {
                variant: value.to_string(),
            }
        }
        FieldKind::Vector { dim } => {
            let array = value
                .as_array()
                .ok_or_else(|| type_error(value, field, field_path))?;
            if array.len() != *dim {
                return Err(AdapterLineError {
                    message: format!(
                        "vector field {field_path} expected dim {dim}, got {}",
                        array.len()
                    ),
                    field_path: field_path.to_string(),
                    expected_kind: format!("vector[{dim}]"),
                    actual_kind: format!("vector[{}]", array.len()),
                });
            }
            let mut out = Vec::with_capacity(*dim);
            for (idx, item) in array.iter().enumerate() {
                let value = item.as_f64().ok_or_else(|| AdapterLineError {
                    message: format!("vector field {field_path}[{idx}] is not numeric"),
                    field_path: format!("{field_path}[{idx}]"),
                    expected_kind: "f64".to_string(),
                    actual_kind: actual_json_kind(item),
                })?;
                out.push(value as f32);
            }
            FieldValue::Vector(out)
        }
    };
    coerced
        .validate(field_path)
        .map_err(|err| AdapterLineError {
            message: err.to_string(),
            field_path: field_path.to_string(),
            expected_kind: expected_kind_name(&field.kind),
            actual_kind: field_value_kind(&coerced).to_string(),
        })?;
    Ok(coerced)
}

fn type_error(value: &Value, field: &FieldSpec, field_path: &str) -> AdapterLineError {
    AdapterLineError {
        message: format!(
            "value at {field_path} has kind {}, expected {}",
            actual_json_kind(value),
            expected_kind_name(&field.kind)
        ),
        field_path: field_path.to_string(),
        expected_kind: expected_kind_name(&field.kind),
        actual_kind: actual_json_kind(value),
    }
}

fn validate_numeric_bounds(
    value: f64,
    field: &FieldSpec,
    field_path: &str,
) -> Result<(), AdapterLineError> {
    if let Some(min) = field.min {
        if value < min {
            return Err(AdapterLineError {
                message: format!("value {value} is below min {min} for {field_path}"),
                field_path: field_path.to_string(),
                expected_kind: format!(">= {min}"),
                actual_kind: "number".to_string(),
            });
        }
    }
    if let Some(max) = field.max {
        if value > max {
            return Err(AdapterLineError {
                message: format!("value {value} exceeds max {max} for {field_path}"),
                field_path: field_path.to_string(),
                expected_kind: format!("<= {max}"),
                actual_kind: "number".to_string(),
            });
        }
    }
    Ok(())
}

fn ensure_required_schema_fields(
    domain_pack: &DomainPack,
    scope: &str,
    fields: &BTreeMap<String, FieldValue>,
) -> Result<(), AdapterLineError> {
    let schema_fields = match scope {
        "state" => &domain_pack.state_schema.fields,
        "action" => &domain_pack.action_schema.fields,
        "outcome" => &domain_pack.outcome_schema.fields,
        _ => return Ok(()),
    };
    for field in schema_fields.iter().filter(|field| field.required) {
        if !fields.contains_key(&field.name) {
            return Err(AdapterLineError {
                message: format!("required field {scope}.{} was not produced", field.name),
                field_path: format!("{scope}.{}", field.name),
                expected_kind: expected_kind_name(&field.kind),
                actual_kind: "missing".to_string(),
            });
        }
    }
    Ok(())
}

fn ensure_time_field(
    domain_pack: &DomainPack,
    fields: &BTreeMap<String, FieldValue>,
) -> Result<(), AdapterLineError> {
    let field = &domain_pack.time_schema.field;
    if field.required && !fields.contains_key(&field.name) {
        return Err(AdapterLineError {
            message: format!("required field time.{} was not produced", field.name),
            field_path: format!("time.{}", field.name),
            expected_kind: expected_kind_name(&field.kind),
            actual_kind: "missing".to_string(),
        });
    }
    Ok(())
}

fn adapted_timestamp_ms(domain_pack: &DomainPack, adapted: &AdaptedLine) -> DynamicJepaResult<i64> {
    let field = &domain_pack.time_schema.field;
    let value =
        adapted
            .time_fields
            .get(&field.name)
            .ok_or_else(|| DynamicJepaError::AdapterFailed {
                adapter_id: "json_event".to_string(),
                event_id: Uuid::nil(),
                message: format!("time field {} missing after adapter success", field.name),
                field_path: format!("time.{}", field.name),
            })?;
    match value {
        FieldValue::I64(value) | FieldValue::UnixMs(value) => Ok(*value),
        other => Err(DynamicJepaError::AdapterFailed {
            adapter_id: "json_event".to_string(),
            event_id: Uuid::nil(),
            message: format!(
                "time field {} must be i64/unix_ms, got {}",
                field.name,
                field_value_kind(other)
            ),
            field_path: format!("time.{}", field.name),
        }),
    }
}

fn actual_json_kind(value: &Value) -> String {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(number) if number.as_i64().is_some() => "i64",
        Value::Number(_) => "f64",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
    .to_string()
}

fn expected_kind_name(kind: &FieldKind) -> String {
    match kind {
        FieldKind::I64 => "i64".to_string(),
        FieldKind::F64 => "f64".to_string(),
        FieldKind::Bool => "bool".to_string(),
        FieldKind::String => "string".to_string(),
        FieldKind::UnixMs => "unix_ms".to_string(),
        FieldKind::Categorical { variants } => format!("categorical{{{}}}", variants.join(",")),
        FieldKind::Vector { dim } => format!("vector[{dim}]"),
    }
}

fn field_value_kind(value: &FieldValue) -> &'static str {
    match value {
        FieldValue::I64(_) => "i64",
        FieldValue::F64(_) => "f64",
        FieldValue::Bool(_) => "bool",
        FieldValue::String(_) => "string",
        FieldValue::Categorical { .. } => "categorical",
        FieldValue::Vector(_) => "vector",
        FieldValue::UnixMs(_) => "unix_ms",
    }
}

fn verify_ingest_created_ids(
    db: &rocksdb::DB,
    created: &IngestCreatedIds,
) -> DynamicJepaResult<()> {
    for id in &created.raw_event_ids {
        let id = parse_event_id(id, "verify_ingest.raw_event_id")?;
        get_raw_event(db, id)?.ok_or_else(|| DynamicJepaError::SourceOfTruthMissing {
            cf: CF_DJ_RAW_EVENTS.to_string(),
            key: id.into_bytes().to_vec(),
        })?;
    }
    for id in &created.adapter_run_ids {
        let id = parse_uuid(id, "verify_ingest.adapter_run_id")?;
        get_adapter_run(db, id)?.ok_or_else(|| DynamicJepaError::SourceOfTruthMissing {
            cf: CF_DJ_ADAPTER_RUNS.to_string(),
            key: id.as_bytes().to_vec(),
        })?;
    }
    for id in &created.failed_adapter_run_ids {
        let id = parse_uuid(id, "verify_ingest.failed_adapter_run_id")?;
        let run =
            get_adapter_run(db, id)?.ok_or_else(|| DynamicJepaError::SourceOfTruthMissing {
                cf: CF_DJ_ADAPTER_RUNS.to_string(),
                key: id.as_bytes().to_vec(),
            })?;
        if run.status != AdapterRunStatus::Failed {
            return Err(DynamicJepaError::StorageInvariantViolation {
                message: format!("adapter run {id} was expected Failed, got {:?}", run.status),
            });
        }
    }
    for id in &created.state_ids {
        let id = parse_state_id(id, "verify_ingest.state_id")?;
        get_normalized_state(db, id)?.ok_or_else(|| DynamicJepaError::SourceOfTruthMissing {
            cf: CF_DJ_NORMALIZED_STATES.to_string(),
            key: id.into_bytes().to_vec(),
        })?;
    }
    for id in &created.action_ids {
        let id = parse_action_id(id, "verify_ingest.action_id")?;
        get_action(db, id)?.ok_or_else(|| DynamicJepaError::SourceOfTruthMissing {
            cf: CF_DJ_ACTIONS.to_string(),
            key: id.into_bytes().to_vec(),
        })?;
    }
    for id in &created.outcome_ids {
        let id = parse_outcome_id(id, "verify_ingest.outcome_id")?;
        get_outcome(db, id)?.ok_or_else(|| DynamicJepaError::SourceOfTruthMissing {
            cf: CF_DJ_OUTCOMES.to_string(),
            key: id.into_bytes().to_vec(),
        })?;
    }
    for id in &created.transition_ids {
        let id = parse_transition_id(id, "verify_ingest.transition_id")?;
        get_transition(db, id)?.ok_or_else(|| DynamicJepaError::SourceOfTruthMissing {
            cf: CF_DJ_TRANSITIONS.to_string(),
            key: id.into_bytes().to_vec(),
        })?;
    }
    Ok(())
}

fn materialize_one_panel(
    db: &rocksdb::DB,
    domain_pack: &DomainPack,
    transition: StateTransition,
) -> DynamicJepaResult<MaterializedPanelEvidence> {
    let state = get_normalized_state(db, transition.prior_state)?.ok_or_else(|| {
        DynamicJepaError::SourceOfTruthMissing {
            cf: CF_DJ_NORMALIZED_STATES.to_string(),
            key: transition.prior_state.into_bytes().to_vec(),
        }
    })?;
    let action = get_action(db, transition.action)?.ok_or_else(|| {
        DynamicJepaError::SourceOfTruthMissing {
            cf: CF_DJ_ACTIONS.to_string(),
            key: transition.action.into_bytes().to_vec(),
        }
    })?;
    let outcome = get_outcome(db, transition.outcome)?.ok_or_else(|| {
        DynamicJepaError::SourceOfTruthMissing {
            cf: CF_DJ_OUTCOMES.to_string(),
            key: transition.outcome.into_bytes().to_vec(),
        }
    })?;

    let mut readings = Vec::with_capacity(domain_pack.instrument_specs.len());
    let mut reading_ids = Vec::with_capacity(domain_pack.instrument_specs.len());
    let mut slots = Vec::with_capacity(domain_pack.instrument_specs.len());
    let mut vectors = Vec::with_capacity(domain_pack.instrument_specs.len());
    let mut masks = Vec::with_capacity(domain_pack.instrument_specs.len());
    for spec in &domain_pack.instrument_specs {
        let slot = panel_slot_for_spec(spec)?;
        match execute_instrument(domain_pack, spec, &state, &action, &outcome, &transition) {
            Ok(output) => {
                let reading_id = Uuid::new_v4();
                let mut reading = InstrumentReading {
                    header: DjRecordHeader::new(
                        reading_id,
                        INSTRUMENT_READING_RECORD_VERSION,
                        domain_pack.id.clone(),
                        domain_pack.version.clone(),
                        now_unix_ms()?,
                        transition.header.source_run_id,
                    ),
                    reading_id,
                    event_id: state.source_event_id,
                    instrument_id: spec.instrument_id.clone(),
                    instrument_hash: instrument_hash(spec),
                    input_hash: output.input_hash,
                    output_dense: output.values.clone(),
                    status: ReadingStatus::Ok,
                };
                reading.refresh_content_hash()?;
                reading_ids.push(reading_id);
                slots.push(slot);
                vectors.push(output.values);
                masks.push(true);
                readings.push(reading);
            }
            Err(err) if !spec.required => {
                let reading_id = Uuid::new_v4();
                let error_message = err.to_string();
                let failure_hash = sha256(
                    format!(
                        "{}:{}:{error_message}",
                        spec.instrument_id, state.source_event_id
                    )
                    .as_bytes(),
                );
                let mut reading = InstrumentReading {
                    header: DjRecordHeader::new(
                        reading_id,
                        INSTRUMENT_READING_RECORD_VERSION,
                        domain_pack.id.clone(),
                        domain_pack.version.clone(),
                        now_unix_ms()?,
                        transition.header.source_run_id,
                    ),
                    reading_id,
                    event_id: state.source_event_id,
                    instrument_id: spec.instrument_id.clone(),
                    instrument_hash: instrument_hash(spec),
                    input_hash: failure_hash,
                    output_dense: Vec::new(),
                    status: ReadingStatus::Failed {
                        error: error_message,
                    },
                };
                reading.refresh_content_hash()?;
                reading_ids.push(reading_id);
                slots.push(slot);
                vectors.push(Vec::new());
                masks.push(false);
                readings.push(reading);
            }
            Err(err) => {
                let audit = build_audit(
                    "materialize_panel",
                    AuditStatus::Failed {
                        error_code: err.code().to_string(),
                    },
                    vec![transition.transition_id.to_string()],
                    Vec::new(),
                    vec![CF_DJ_AUDIT_LOG.to_string()],
                    Vec::new(),
                )?;
                put_audit_record(db, &audit)?;
                return Err(err);
            }
        }
    }

    let panel_id = PanelId::new_v4();
    let panel_hash = panel_hash(domain_pack, &reading_ids, &slots, &vectors)?;
    let mut panel = LatentPanel {
        header: DjRecordHeader::new(
            panel_id.0,
            LATENT_PANEL_RECORD_VERSION,
            domain_pack.id.clone(),
            domain_pack.version.clone(),
            now_unix_ms()?,
            transition.header.source_run_id,
        ),
        panel_id,
        event_id: state.source_event_id,
        state_id: state.state_id,
        action_id: action.action_id,
        outcome_id: Some(outcome.outcome_id),
        instrument_reading_ids: reading_ids.clone(),
        ordered_slots: slots,
        slot_vectors: vectors,
        slot_masks: masks,
        panel_hash,
        materializer_version: 1,
    };
    panel.refresh_content_hash()?;
    let audit = build_audit(
        "materialize_panel",
        AuditStatus::Ok,
        vec![transition.transition_id.to_string()],
        std::iter::once(panel_id.to_string())
            .chain(reading_ids.iter().map(Uuid::to_string))
            .collect(),
        vec![
            CF_DJ_INSTRUMENT_READINGS.to_string(),
            CF_DJ_LATENT_PANELS.to_string(),
            CF_DJ_AUDIT_LOG.to_string(),
        ],
        std::iter::once(panel.header.content_hash)
            .chain(readings.iter().map(|reading| reading.header.content_hash))
            .collect(),
    )?;
    put_panel_with_readings_batch(db, &readings, &panel, &audit)?;

    let stored_panel =
        get_latent_panel(db, panel_id)?.ok_or_else(|| DynamicJepaError::SourceOfTruthMissing {
            cf: CF_DJ_LATENT_PANELS.to_string(),
            key: panel_id.into_bytes().to_vec(),
        })?;
    if stored_panel != panel {
        return Err(DynamicJepaError::StorageInvariantViolation {
            message: format!("latent panel readback mismatch for panel {panel_id}"),
        });
    }
    let stored_readings = list_instrument_readings_for_event(db, state.source_event_id)?;
    if stored_readings.len() != domain_pack.instrument_specs.len() {
        return Err(DynamicJepaError::StorageInvariantViolation {
            message: format!(
                "event {} expected {} instrument readings, found {}",
                state.source_event_id,
                domain_pack.instrument_specs.len(),
                stored_readings.len()
            ),
        });
    }
    Ok(MaterializedPanelEvidence {
        panel_id: panel_id.to_string(),
        event_id: state.source_event_id.to_string(),
        transition_id: transition.transition_id.to_string(),
        instrument_reading_ids: reading_ids.iter().map(Uuid::to_string).collect(),
    })
}

struct InstrumentOutput {
    values: Vec<f32>,
    input_hash: [u8; 32],
}

fn execute_instrument(
    domain_pack: &DomainPack,
    spec: &InstrumentSpec,
    state: &NormalizedState,
    action: &NormalizedAction,
    outcome: &NormalizedOutcome,
    transition: &StateTransition,
) -> DynamicJepaResult<InstrumentOutput> {
    let inputs = spec
        .input_fields
        .iter()
        .map(|path| {
            resolve_field_value(domain_pack, path, state, action, outcome, transition)
                .map(|value| (path.clone(), value))
        })
        .collect::<DynamicJepaResult<Vec<_>>>()?;
    let input_hash = sha256(&serde_json::to_vec(&inputs).map_err(|err| {
        DynamicJepaError::validation(
            "instrument.input_hash",
            format!("failed to serialize instrument inputs: {err}"),
            "instrument inputs must be JSON serializable for deterministic hashing",
        )
    })?);
    let values = match &spec.kind {
        InstrumentKind::Scalar => {
            if inputs.len() != 1 {
                return Err(instrument_failed(
                    spec,
                    state.source_event_id,
                    "scalar instrument requires exactly one input field",
                    spec.input_fields.join(","),
                ));
            }
            let raw = numeric_field_value(&inputs[0].1).ok_or_else(|| {
                instrument_failed(
                    spec,
                    state.source_event_id,
                    format!(
                        "scalar input {} is {} not numeric",
                        inputs[0].0,
                        field_value_kind(&inputs[0].1)
                    ),
                    inputs[0].0.clone(),
                )
            })?;
            validate_instrument_numeric(spec, raw, &inputs[0].0, state.source_event_id)?;
            vec![normalize_value(raw, &spec.normalization)? as f32]
        }
        InstrumentKind::Categorical { variants } => {
            if inputs.len() != 1 {
                return Err(instrument_failed(
                    spec,
                    state.source_event_id,
                    "categorical instrument requires exactly one input field",
                    spec.input_fields.join(","),
                ));
            }
            let variant = match &inputs[0].1 {
                FieldValue::Categorical { variant } | FieldValue::String(variant) => variant,
                other => {
                    return Err(instrument_failed(
                        spec,
                        state.source_event_id,
                        format!(
                            "categorical input {} is {} not categorical/string",
                            inputs[0].0,
                            field_value_kind(other)
                        ),
                        inputs[0].0.clone(),
                    ));
                }
            };
            let idx = variants
                .iter()
                .position(|item| item == variant)
                .ok_or_else(|| {
                    instrument_failed(
                        spec,
                        state.source_event_id,
                        format!("variant {variant:?} is not declared"),
                        inputs[0].0.clone(),
                    )
                })?;
            onehot(variants.len(), idx)?
        }
        InstrumentKind::Onehot { dim } => {
            let idx = compound_onehot_index(*dim, &inputs, spec, state.source_event_id)?;
            onehot(*dim, idx)?
        }
        InstrumentKind::Time { encoding } => {
            if inputs.len() != 1 {
                return Err(instrument_failed(
                    spec,
                    state.source_event_id,
                    "time instrument requires exactly one input field",
                    spec.input_fields.join(","),
                ));
            }
            let raw = numeric_field_value(&inputs[0].1).ok_or_else(|| {
                instrument_failed(
                    spec,
                    state.source_event_id,
                    format!(
                        "time input {} is {} not numeric",
                        inputs[0].0,
                        field_value_kind(&inputs[0].1)
                    ),
                    inputs[0].0.clone(),
                )
            })?;
            validate_instrument_numeric(spec, raw, &inputs[0].0, state.source_event_id)?;
            match encoding {
                TimeEncoding::UnixSeconds => vec![(raw / 1000.0) as f32],
                TimeEncoding::FractionalDayOfWeek => {
                    let seconds_in_week = 7.0 * 24.0 * 3600.0;
                    vec![((raw / 1000.0) % seconds_in_week / seconds_in_week) as f32]
                }
            }
        }
    };
    let expected_dim = spec.output_shape.iter().product::<usize>();
    if values.len() != expected_dim {
        return Err(DynamicJepaError::PanelShapeMismatch {
            domain_pack_id: domain_pack.id.to_string(),
            expected: vec![expected_dim],
            actual: vec![values.len()],
        });
    }
    for (idx, value) in values.iter().enumerate() {
        if !value.is_finite() {
            return Err(instrument_failed(
                spec,
                state.source_event_id,
                format!("output value {idx} is not finite: {value}"),
                spec.input_fields.join(","),
            ));
        }
    }
    Ok(InstrumentOutput { values, input_hash })
}

fn resolve_field_value(
    domain_pack: &DomainPack,
    path: &str,
    state: &NormalizedState,
    action: &NormalizedAction,
    outcome: &NormalizedOutcome,
    transition: &StateTransition,
) -> DynamicJepaResult<FieldValue> {
    let (scope, name) = path.split_once('.').ok_or_else(|| {
        DynamicJepaError::validation(
            "instrument.input_fields",
            format!("input field {path:?} is not dotted"),
            "use state/action/outcome/time dotted instrument input fields",
        )
    })?;
    let value = match scope {
        "state" => state.fields.get(name).cloned(),
        "action" => action.fields.get(name).cloned(),
        "outcome" => outcome.fields.get(name).cloned(),
        "time" if domain_pack.time_schema.field.name == name => {
            match domain_pack.time_schema.field.kind {
                FieldKind::UnixMs => Some(FieldValue::UnixMs(transition.timestamp_ms)),
                _ => Some(FieldValue::I64(transition.timestamp_ms)),
            }
        }
        _ => None,
    };
    value.ok_or_else(|| DynamicJepaError::InstrumentFailed {
        instrument_id: "unknown".to_string(),
        event_id: state.source_event_id.0,
        message: format!("input field {path:?} missing from normalized records"),
        field_path: path.to_string(),
    })
}

fn numeric_field_value(value: &FieldValue) -> Option<f64> {
    match value {
        FieldValue::I64(value) | FieldValue::UnixMs(value) => Some(*value as f64),
        FieldValue::F64(value) => Some(*value),
        _ => None,
    }
}

fn validate_instrument_numeric(
    spec: &InstrumentSpec,
    value: f64,
    field_path: &str,
    event_id: EventId,
) -> DynamicJepaResult<()> {
    if spec.validation.require_finite && !value.is_finite() {
        return Err(instrument_failed(
            spec,
            event_id,
            format!("numeric input {field_path} is not finite: {value}"),
            field_path.to_string(),
        ));
    }
    if spec.validation.reject_nan && value.is_nan() {
        return Err(instrument_failed(
            spec,
            event_id,
            format!("numeric input {field_path} is NaN"),
            field_path.to_string(),
        ));
    }
    if spec.validation.reject_inf && value.is_infinite() {
        return Err(instrument_failed(
            spec,
            event_id,
            format!("numeric input {field_path} is infinite"),
            field_path.to_string(),
        ));
    }
    if let Some(min) = spec.validation.min {
        if value < min {
            return Err(instrument_failed(
                spec,
                event_id,
                format!("numeric input {field_path}={value} is below min {min}"),
                field_path.to_string(),
            ));
        }
    }
    if let Some(max) = spec.validation.max {
        if value > max {
            return Err(instrument_failed(
                spec,
                event_id,
                format!("numeric input {field_path}={value} exceeds max {max}"),
                field_path.to_string(),
            ));
        }
    }
    Ok(())
}

fn normalize_value(value: f64, normalization: &Normalization) -> DynamicJepaResult<f64> {
    match normalization {
        Normalization::None => Ok(value),
        Normalization::StandardScore { mean, std } => {
            if *std <= 0.0 || !std.is_finite() {
                return Err(DynamicJepaError::validation(
                    "instrument.normalization.std",
                    format!("std must be finite and > 0, got {std}"),
                    "fix the registered domain pack normalization",
                ));
            }
            Ok((value - *mean) / *std)
        }
    }
}

fn compound_onehot_index(
    dim: usize,
    inputs: &[(String, FieldValue)],
    spec: &InstrumentSpec,
    event_id: EventId,
) -> DynamicJepaResult<usize> {
    if inputs.len() == 1 {
        let value = numeric_field_value(&inputs[0].1).ok_or_else(|| {
            instrument_failed(
                spec,
                event_id,
                format!("onehot input {} is not numeric", inputs[0].0),
                inputs[0].0.clone(),
            )
        })?;
        let idx = value as i64;
        if idx < 0 || idx as usize >= dim || (idx as f64 - value).abs() > f64::EPSILON {
            return Err(instrument_failed(
                spec,
                event_id,
                format!("onehot index {value} is outside 0..{}", dim - 1),
                inputs[0].0.clone(),
            ));
        }
        return Ok(idx as usize);
    }
    if inputs.len() == 2 {
        let side = (dim as f64).sqrt() as usize;
        if side * side != dim {
            return Err(instrument_failed(
                spec,
                event_id,
                format!("compound two-field onehot dim {dim} is not a square"),
                spec.input_fields.join(","),
            ));
        }
        let x = numeric_field_value(&inputs[0].1).ok_or_else(|| {
            instrument_failed(
                spec,
                event_id,
                format!("onehot input {} is not numeric", inputs[0].0),
                inputs[0].0.clone(),
            )
        })? as i64;
        let y = numeric_field_value(&inputs[1].1).ok_or_else(|| {
            instrument_failed(
                spec,
                event_id,
                format!("onehot input {} is not numeric", inputs[1].0),
                inputs[1].0.clone(),
            )
        })? as i64;
        if x < 0 || y < 0 || x as usize >= side || y as usize >= side {
            return Err(instrument_failed(
                spec,
                event_id,
                format!(
                    "compound onehot coordinate ({x},{y}) is outside 0..{}",
                    side - 1
                ),
                spec.input_fields.join(","),
            ));
        }
        return Ok(x as usize * side + y as usize);
    }
    Err(instrument_failed(
        spec,
        event_id,
        format!(
            "onehot instrument supports one index input or two coordinate inputs, got {}",
            inputs.len()
        ),
        spec.input_fields.join(","),
    ))
}

fn onehot(dim: usize, idx: usize) -> DynamicJepaResult<Vec<f32>> {
    if idx >= dim {
        return Err(DynamicJepaError::PanelShapeMismatch {
            domain_pack_id: "onehot".to_string(),
            expected: vec![dim],
            actual: vec![idx + 1],
        });
    }
    let mut values = vec![0.0f32; dim];
    values[idx] = 1.0;
    Ok(values)
}

fn instrument_failed(
    spec: &InstrumentSpec,
    event_id: EventId,
    message: impl Into<String>,
    field_path: impl Into<String>,
) -> DynamicJepaError {
    DynamicJepaError::InstrumentFailed {
        instrument_id: spec.instrument_id.to_string(),
        event_id: event_id.0,
        message: message.into(),
        field_path: field_path.into(),
    }
}

fn panel_slot_for_spec(spec: &InstrumentSpec) -> DynamicJepaResult<PanelSlot> {
    let first = spec.input_fields.first().ok_or_else(|| {
        DynamicJepaError::validation(
            "InstrumentSpec.input_fields",
            "instrument must have at least one input field",
            "fix the registered domain pack instrument spec",
        )
    })?;
    let kind = if first.starts_with("state.") {
        PanelSlotKind::State
    } else if first.starts_with("action.") {
        PanelSlotKind::Action
    } else if first.starts_with("outcome.") {
        PanelSlotKind::Outcome
    } else if first.starts_with("time.") {
        PanelSlotKind::Time
    } else {
        return Err(DynamicJepaError::validation(
            "InstrumentSpec.input_fields",
            format!("unsupported panel slot input prefix in {first:?}"),
            "use state/action/outcome/time input fields for Phase 4 panels",
        ));
    };
    Ok(PanelSlot {
        instrument_id: spec.instrument_id.clone(),
        dim: spec.output_shape.iter().product(),
        kind,
    })
}

fn instrument_hash(spec: &InstrumentSpec) -> [u8; 16] {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(spec.instrument_id.as_str().as_bytes());
    bytes.push(0);
    bytes.push(spec.version);
    bytes.push(0);
    bytes.extend_from_slice(format!("{:?}", spec.output_shape).as_bytes());
    context_graph_storage::dynamicjepa::keys::blake3_128(&bytes)
}

fn panel_hash(
    domain_pack: &DomainPack,
    reading_ids: &[Uuid],
    slots: &[PanelSlot],
    vectors: &[Vec<f32>],
) -> DynamicJepaResult<[u8; 32]> {
    let mut sorted_reading_ids = reading_ids.iter().map(Uuid::to_string).collect::<Vec<_>>();
    sorted_reading_ids.sort();
    let bytes = serde_json::to_vec(&json!({
        "domain_pack_id": domain_pack.id.to_string(),
        "reading_ids": sorted_reading_ids,
        "ordered_slots": slots,
        "slot_vectors": vectors,
    }))
    .map_err(|err| {
        DynamicJepaError::validation(
            "panel_hash",
            format!("failed to serialize panel hash payload: {err}"),
            "panel hash payload must remain JSON serializable",
        )
    })?;
    Ok(sha256(&bytes))
}

fn panel_for_transition(
    db: &rocksdb::DB,
    transition: &StateTransition,
) -> DynamicJepaResult<Option<LatentPanel>> {
    Ok(list_latent_panels(db, 1_000_000, 0)?
        .into_iter()
        .find(|panel| {
            panel.state_id == transition.prior_state
                && panel.action_id == transition.action
                && panel.outcome_id == Some(transition.outcome)
        }))
}

fn phase4_ingest_decoded_records(db: &rocksdb::DB) -> DynamicJepaResult<Value> {
    Ok(json!({
        CF_DJ_RAW_EVENTS: inspect_cf(db, CF_DJ_RAW_EVENTS, 20, 0)?,
        CF_DJ_ADAPTER_RUNS: inspect_cf(db, CF_DJ_ADAPTER_RUNS, 20, 0)?,
        CF_DJ_NORMALIZED_STATES: inspect_cf(db, CF_DJ_NORMALIZED_STATES, 20, 0)?,
        CF_DJ_ACTIONS: inspect_cf(db, CF_DJ_ACTIONS, 20, 0)?,
        CF_DJ_OUTCOMES: inspect_cf(db, CF_DJ_OUTCOMES, 20, 0)?,
        CF_DJ_TRANSITIONS: inspect_cf(db, CF_DJ_TRANSITIONS, 20, 0)?,
        CF_DJ_AUDIT_LOG: inspect_cf(db, CF_DJ_AUDIT_LOG, 40, 0)?,
    }))
}

fn phase4_panel_decoded_records(db: &rocksdb::DB) -> DynamicJepaResult<Value> {
    Ok(json!({
        CF_DJ_INSTRUMENT_READINGS: inspect_cf(db, CF_DJ_INSTRUMENT_READINGS, 40, 0)?,
        CF_DJ_LATENT_PANELS: inspect_cf(db, CF_DJ_LATENT_PANELS, 40, 0)?,
        CF_DJ_AUDIT_LOG: inspect_cf(db, CF_DJ_AUDIT_LOG, 80, 0)?,
    }))
}

fn enrich_phase4_error_value(
    value: &mut Value,
    before_counts: &BTreeMap<String, u64>,
    after_counts: &BTreeMap<String, u64>,
    created_ids: Value,
    decoded_records: Value,
) -> DynamicJepaResult<()> {
    if let Some(object) = value.as_object_mut() {
        object.insert(
            "count_delta".to_string(),
            json!(count_delta(before_counts, after_counts)?),
        );
        object.insert("created_ids".to_string(), created_ids);
        object.insert("decoded_records".to_string(), decoded_records);
    }
    Ok(())
}

fn build_audit(
    operation: impl Into<String>,
    status: AuditStatus,
    input_ids: Vec<String>,
    output_ids: Vec<String>,
    cfs_touched: Vec<String>,
    content_hashes: Vec<[u8; 32]>,
) -> DynamicJepaResult<DjAuditRecord> {
    let timestamp_unix_nanos = now_unix_ms()? as u64 * 1_000_000;
    let record = DjAuditRecord {
        audit_id: Uuid::new_v4(),
        timestamp_unix_nanos,
        operation: operation.into(),
        actor: "cli".to_string(),
        input_ids,
        output_ids,
        cfs_touched,
        content_hashes,
        status,
        verification_run_id: None,
    };
    record.validate()?;
    Ok(record)
}

fn parse_uuid(value: &str, field: &str) -> DynamicJepaResult<Uuid> {
    Uuid::parse_str(value).map_err(|err| {
        DynamicJepaError::validation(
            field,
            format!("invalid UUID {value:?}: {err}"),
            "pass an id returned by a prior DynamicJEPA command or decoded source-of-truth row",
        )
    })
}

fn parse_event_id(value: &str, field: &str) -> DynamicJepaResult<EventId> {
    Ok(EventId(parse_uuid(value, field)?))
}

fn parse_state_id(value: &str, field: &str) -> DynamicJepaResult<StateId> {
    Ok(StateId(parse_uuid(value, field)?))
}

fn parse_action_id(value: &str, field: &str) -> DynamicJepaResult<ActionId> {
    Ok(ActionId(parse_uuid(value, field)?))
}

fn parse_outcome_id(value: &str, field: &str) -> DynamicJepaResult<OutcomeId> {
    Ok(OutcomeId(parse_uuid(value, field)?))
}

fn parse_transition_id(value: &str, field: &str) -> DynamicJepaResult<TransitionId> {
    Ok(TransitionId(parse_uuid(value, field)?))
}

fn parse_panel_id(value: &str, field: &str) -> DynamicJepaResult<PanelId> {
    Ok(PanelId(parse_uuid(value, field)?))
}

fn parse_domain_pack_file(path: &Path) -> DynamicJepaResult<DomainPack> {
    let bytes = fs::read(path).map_err(|err| {
        DynamicJepaError::validation(
            "domain_pack.file",
            format!("failed to read domain pack TOML {}: {err}", path.display()),
            "provide an existing TOML file under configs/dynamicjepa/domain_packs",
        )
    })?;
    let source_hash = sha256(&bytes);
    let text = std::str::from_utf8(&bytes).map_err(|err| {
        DynamicJepaError::validation(
            "domain_pack.file",
            format!("domain pack TOML is not UTF-8: {err}"),
            "save the domain pack TOML as UTF-8 text",
        )
    })?;
    let value: toml::Value = toml::from_str(text).map_err(|err| {
        DynamicJepaError::validation(
            "domain_pack.toml",
            format!("TOML parse failed: {err}"),
            "fix the TOML syntax before registration; no partial registry rows are written",
        )
    })?;
    reject_env_substitution(&value, "domain_pack")?;

    let parsed: DomainPackToml = toml::from_str(text).map_err(|err| {
        DynamicJepaError::validation(
            "domain_pack.toml",
            format!("strict TOML schema parse failed: {err}"),
            "remove unknown keys or extend the parser intentionally before registering this pack",
        )
    })?;
    let domain_id = DomainPackId::new(parsed.domain.id)?;
    let version = parsed.domain.version;
    let header = DjRecordHeader::new(
        Uuid::new_v4(),
        DOMAIN_PACK_RECORD_VERSION,
        domain_id.clone(),
        version.clone(),
        now_unix_ms()?,
        Some(Uuid::new_v4()),
    );
    let candidate_actions = candidate_actions_from_toml(parsed.planner_policy.candidate_actions)?;
    let mut pack = DomainPack {
        header,
        id: domain_id,
        version,
        title: parsed.domain.title,
        schema_version: parsed.domain.schema_version,
        state_schema: StateSchema {
            fields: fields_from_toml(parsed.state.fields, "state")?,
        },
        action_schema: ActionSchema {
            fields: fields_from_toml(parsed.action.fields, "action")?,
        },
        outcome_schema: OutcomeSchema {
            fields: fields_from_toml(parsed.outcome.fields, "outcome")?,
        },
        entity_schema: EntitySchema {
            fields: fields_from_toml(parsed.entity.fields, "entity")?,
        },
        time_schema: TimeSchema {
            field: field_spec_from_toml(parsed.time.field, "time.field")?,
        },
        instrument_specs: parsed
            .instruments
            .into_iter()
            .enumerate()
            .map(|(idx, spec)| instrument_from_toml(spec, idx))
            .collect::<DynamicJepaResult<Vec<_>>>()?,
        adapter_specs: parsed
            .adapters
            .into_iter()
            .map(adapter_from_toml)
            .collect::<DynamicJepaResult<Vec<_>>>()?,
        objective_specs: parsed
            .objectives
            .into_iter()
            .map(|objective| ObjectiveSpec {
                id: objective.id,
                kind: objective.kind,
                input_panel: objective.input_panel,
                target: objective.target,
                loss_weight: objective.loss_weight,
                required_dataset_fields: objective.required_dataset_fields,
            })
            .collect(),
        invariants: parsed
            .invariants
            .into_iter()
            .map(|invariant| InvariantSpec {
                id: invariant.id,
                expression: invariant.expression,
                severity: invariant.severity,
            })
            .collect(),
        dataset_policy: DatasetPolicy {
            default_split: parsed.dataset_policy.default_split,
            split_key: parsed.dataset_policy.split_key,
            split_buckets: parsed.dataset_policy.split_buckets,
            default_objective: parsed.dataset_policy.default_objective,
            negative_sampling: parsed.dataset_policy.negative_sampling,
            min_negatives_per_row: parsed.dataset_policy.min_negatives_per_row,
        },
        planner_policy: PlannerPolicy {
            candidate_actions,
            guards: parsed.planner_policy.guards,
            surprise_threshold: parsed.planner_policy.surprise_threshold,
        },
        verification_policy: VerificationPolicy {
            required_cfs: parsed.verification_policy.required_cfs,
            expected_event_count: parsed.verification_policy.expected_event_count,
            expected_panel_count: parsed.verification_policy.expected_panel_count,
            expected_dataset_row_count: parsed.verification_policy.expected_dataset_row_count,
            expected_trajectory_count: parsed.verification_policy.expected_trajectory_count,
            expected_train_rows_min: parsed.verification_policy.expected_train_rows_min,
            expected_train_rows_max: parsed.verification_policy.expected_train_rows_max,
            expected_val_rows_min: parsed.verification_policy.expected_val_rows_min,
            expected_val_rows_max: parsed.verification_policy.expected_val_rows_max,
            expected_test_rows_min: parsed.verification_policy.expected_test_rows_min,
            expected_test_rows_max: parsed.verification_policy.expected_test_rows_max,
        },
        source_hash,
    };
    pack.refresh_content_hash()?;
    pack.validate_record()?;
    Ok(pack)
}

fn instrument_from_toml(spec: InstrumentToml, idx: usize) -> DynamicJepaResult<InstrumentSpec> {
    Ok(InstrumentSpec {
        instrument_id: InstrumentId::new(spec.id)?,
        kind: instrument_kind_from_toml(spec.kind, &format!("instruments[{idx}].kind"))?,
        input_fields: spec.input_fields,
        output_shape: spec.output_shape,
        normalization: normalization_from_toml(
            spec.normalization,
            &format!("instruments[{idx}].normalization"),
        )?,
        required: spec.required,
        model_ref: spec.model_ref,
        version: spec.version,
        validation: InstrumentValidation {
            require_finite: spec.validation.require_finite,
            min: spec.validation.min,
            max: spec.validation.max,
            reject_nan: spec.validation.reject_nan,
            reject_inf: spec.validation.reject_inf,
        },
    })
}

fn adapter_from_toml(adapter: AdapterToml) -> DynamicJepaResult<AdapterSpec> {
    Ok(AdapterSpec {
        adapter_id: AdapterId::new(adapter.id)?,
        kind: adapter.kind,
        version: adapter.version,
        mapping: adapter.mapping,
    })
}

fn field_spec_from_toml(field: FieldToml, path: &str) -> DynamicJepaResult<FieldSpec> {
    Ok(FieldSpec {
        name: field.name,
        kind: field_kind_from_toml(field.kind, &format!("{path}.kind"))?,
        required: field.required,
        min: field.min,
        max: field.max,
    })
}

fn fields_from_toml(fields: Vec<FieldToml>, path: &str) -> DynamicJepaResult<Vec<FieldSpec>> {
    fields
        .into_iter()
        .enumerate()
        .map(|(idx, field)| field_spec_from_toml(field, &format!("{path}.fields[{idx}]")))
        .collect()
}

fn field_kind_from_toml(kind: TomlKind, field: &str) -> DynamicJepaResult<FieldKind> {
    match kind {
        TomlKind::Name(name) => match name.as_str() {
            "i64" => Ok(FieldKind::I64),
            "f64" => Ok(FieldKind::F64),
            "bool" => Ok(FieldKind::Bool),
            "string" => Ok(FieldKind::String),
            "unix_ms" => Ok(FieldKind::UnixMs),
            other => Err(toml_validation_error(
                field,
                format!("unsupported field kind name {other:?}"),
            )),
        },
        TomlKind::Table(table) => match table.kind_type.as_str() {
            "i64" => Ok(FieldKind::I64),
            "f64" => Ok(FieldKind::F64),
            "bool" => Ok(FieldKind::Bool),
            "string" => Ok(FieldKind::String),
            "unix_ms" => Ok(FieldKind::UnixMs),
            "categorical" => Ok(FieldKind::Categorical {
                variants: table.variants.ok_or_else(|| {
                    toml_validation_error(field, "categorical field kind missing variants")
                })?,
            }),
            "vector" => Ok(FieldKind::Vector {
                dim: table
                    .dim
                    .ok_or_else(|| toml_validation_error(field, "vector field kind missing dim"))?,
            }),
            other => Err(toml_validation_error(
                field,
                format!("unsupported field kind table type {other:?}"),
            )),
        },
    }
}

fn instrument_kind_from_toml(kind: TomlKind, field: &str) -> DynamicJepaResult<InstrumentKind> {
    match kind {
        TomlKind::Name(name) => match name.as_str() {
            "scalar" => Ok(InstrumentKind::Scalar),
            other => Err(toml_validation_error(
                field,
                format!("unsupported instrument kind name {other:?}"),
            )),
        },
        TomlKind::Table(table) => match table.kind_type.as_str() {
            "scalar" => Ok(InstrumentKind::Scalar),
            "categorical" => Ok(InstrumentKind::Categorical {
                variants: table.variants.ok_or_else(|| {
                    toml_validation_error(field, "categorical instrument kind missing variants")
                })?,
            }),
            "onehot" => Ok(InstrumentKind::Onehot {
                dim: table.dim.ok_or_else(|| {
                    toml_validation_error(field, "onehot instrument kind missing dim")
                })?,
            }),
            "time" => {
                let encoding = match table.encoding.as_deref() {
                    Some("fractional_day_of_week") => TimeEncoding::FractionalDayOfWeek,
                    Some("unix_seconds") => TimeEncoding::UnixSeconds,
                    other => {
                        return Err(toml_validation_error(
                            field,
                            format!("unsupported time encoding {other:?}"),
                        ));
                    }
                };
                Ok(InstrumentKind::Time { encoding })
            }
            other => Err(toml_validation_error(
                field,
                format!("unsupported instrument kind table type {other:?}"),
            )),
        },
    }
}

fn normalization_from_toml(
    value: NormalizationToml,
    field: &str,
) -> DynamicJepaResult<Normalization> {
    match value.norm_type.as_str() {
        "none" => Ok(Normalization::None),
        "standard_score" => Ok(Normalization::StandardScore {
            mean: value
                .mean
                .ok_or_else(|| toml_validation_error(field, "standard_score missing mean"))?,
            std: value
                .std
                .ok_or_else(|| toml_validation_error(field, "standard_score missing std"))?,
        }),
        other => Err(toml_validation_error(
            field,
            format!("unsupported normalization type {other:?}"),
        )),
    }
}

fn candidate_actions_from_toml(
    value: CandidateActionsToml,
) -> DynamicJepaResult<CandidateActionConfig> {
    if value.kind != "enumerated" {
        return Err(toml_validation_error(
            "planner_policy.candidate_actions.kind",
            format!("unsupported candidate action kind {:?}", value.kind),
        ));
    }
    match (value.deltas, value.moves) {
        (Some(deltas), None) => Ok(CandidateActionConfig::EnumeratedDeltas { deltas }),
        (None, Some(moves)) => Ok(CandidateActionConfig::EnumeratedMoves { moves }),
        (Some(_), Some(_)) => Err(toml_validation_error(
            "planner_policy.candidate_actions",
            "declare deltas or moves, not both",
        )),
        (None, None) => Err(toml_validation_error(
            "planner_policy.candidate_actions",
            "enumerated candidate actions require deltas or moves",
        )),
    }
}

fn reject_env_substitution(value: &toml::Value, path: &str) -> DynamicJepaResult<()> {
    match value {
        toml::Value::String(text) if text.contains("${") => Err(DynamicJepaError::validation(
            path,
            "environment-variable substitution is forbidden in domain pack string fields",
            "write literal deterministic values and bump the domain pack version if values change",
        )),
        toml::Value::Array(items) => {
            for (idx, item) in items.iter().enumerate() {
                reject_env_substitution(item, &format!("{path}[{idx}]"))?;
            }
            Ok(())
        }
        toml::Value::Table(table) => {
            for (key, item) in table {
                reject_env_substitution(item, &format!("{path}.{key}"))?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn open_store(path: &Path) -> DynamicJepaResult<RocksDbTeleologicalStore> {
    RocksDbTeleologicalStore::open_with_config(path, TeleologicalStoreConfig::default()).map_err(
        |err| DynamicJepaError::Storage {
            operation: "open_dynamicjepa_db".to_string(),
            cf: "rocksdb".to_string(),
            message: format!("failed to open {}: {err}", path.display()),
            remediation: "verify the DB path is a writable demo path and no active process owns the RocksDB lock".to_string(),
        },
    )
}

fn canonical_existing_path(path: &Path, field: &str) -> DynamicJepaResult<String> {
    fs::canonicalize(path)
        .map_err(|err| {
            DynamicJepaError::validation(
                field,
                format!("failed to canonicalize {}: {err}", path.display()),
                "use an existing path; DB paths are created when opened, input files must already exist",
            )
        })
        .map(|path| path.display().to_string())
}

fn resolve_dynamic_cf_name(name: &str) -> DynamicJepaResult<&'static str> {
    DJ_CF_NAMES
        .iter()
        .copied()
        .find(|candidate| *candidate == name)
        .ok_or_else(|| {
            DynamicJepaError::validation(
                "inspect-cf.cf",
                format!("unknown DynamicJEPA column family {name:?}"),
                format!("use one of: {}", DJ_CF_NAMES.join(", ")),
            )
        })
}

fn count_delta(
    before: &BTreeMap<String, u64>,
    after: &BTreeMap<String, u64>,
) -> DynamicJepaResult<BTreeMap<String, i64>> {
    let mut delta = BTreeMap::new();
    for (cf, before_count) in before {
        let after_count =
            after
                .get(cf)
                .copied()
                .ok_or_else(|| DynamicJepaError::StorageInvariantViolation {
                    message: format!("after-count snapshot missing DynamicJEPA column family {cf}"),
                })?;
        delta.insert(cf.clone(), after_count as i64 - *before_count as i64);
    }
    for cf in after.keys() {
        if !before.contains_key(cf) {
            return Err(DynamicJepaError::StorageInvariantViolation {
                message: format!(
                    "after-count snapshot contained unexpected DynamicJEPA column family {cf}"
                ),
            });
        }
    }
    Ok(delta)
}

fn error_value(
    operation: &'static str,
    db_path: Option<String>,
    source_cfs: &[&'static str],
    before_counts: Option<BTreeMap<String, u64>>,
    after_counts: Option<BTreeMap<String, u64>>,
    err: &DynamicJepaError,
) -> Value {
    let mut object = Map::new();
    object.insert("operation".to_string(), json!(operation));
    object.insert("status".to_string(), json!("error"));
    if let Some(db_path) = db_path {
        object.insert("db_path".to_string(), json!(db_path));
    }
    if !source_cfs.is_empty() {
        object.insert("source_of_truth".to_string(), json!(source_cfs));
    }
    if let Some(before_counts) = before_counts {
        object.insert("before_counts".to_string(), json!(before_counts));
    }
    if let Some(after_counts) = after_counts {
        object.insert("after_counts".to_string(), json!(after_counts));
    }
    object.insert("error_code".to_string(), json!(err.code()));
    object.insert("error_message".to_string(), json!(err.to_string()));
    if let Some(remediation) = remediation(err) {
        object.insert("remediation".to_string(), json!(remediation));
    }
    match err {
        DynamicJepaError::Validation { field, .. }
        | DynamicJepaError::SchemaValidationFailed { field, .. } => {
            object.insert("field".to_string(), json!(field));
        }
        DynamicJepaError::Storage { operation, cf, .. } => {
            object.insert("storage_operation".to_string(), json!(operation));
            object.insert("cf".to_string(), json!(cf));
        }
        DynamicJepaError::Codec {
            expected,
            actual,
            payload_type,
            ..
        } => {
            object.insert("expected_version".to_string(), json!(expected));
            object.insert("actual_version".to_string(), json!(actual));
            object.insert("payload_type".to_string(), json!(payload_type));
        }
        DynamicJepaError::SourceOfTruthMissing { cf, key } => {
            object.insert("cf".to_string(), json!(cf));
            object.insert("key_hex".to_string(), json!(hex_bytes(key)));
        }
        DynamicJepaError::AdapterFailed {
            adapter_id,
            event_id,
            field_path,
            ..
        } => {
            object.insert("adapter_id".to_string(), json!(adapter_id));
            object.insert("event_id".to_string(), json!(event_id.to_string()));
            object.insert("field_path".to_string(), json!(field_path));
        }
        DynamicJepaError::InstrumentFailed {
            instrument_id,
            event_id,
            field_path,
            ..
        } => {
            object.insert("instrument_id".to_string(), json!(instrument_id));
            object.insert("event_id".to_string(), json!(event_id.to_string()));
            object.insert("field_path".to_string(), json!(field_path));
        }
        DynamicJepaError::DomainPackVersionMismatch {
            id,
            expected,
            actual,
        } => {
            object.insert("id".to_string(), json!(id));
            object.insert("expected".to_string(), json!(expected));
            object.insert("actual".to_string(), json!(actual));
        }
        _ => {}
    }
    Value::Object(object)
}

fn remediation(err: &DynamicJepaError) -> Option<String> {
    match err {
        DynamicJepaError::Validation { remediation, .. }
        | DynamicJepaError::SchemaValidationFailed { remediation, .. }
        | DynamicJepaError::Storage { remediation, .. }
        | DynamicJepaError::Codec { remediation, .. }
        | DynamicJepaError::TrainingFailed { remediation, .. } => Some(remediation.clone()),
        DynamicJepaError::SourceOfTruthMissing { .. } => Some(
            "inspect DynamicJEPA counts and the referenced CF; rerun the prior command from a fresh demo DB if the row was never written"
                .to_string(),
        ),
        DynamicJepaError::StorageInvariantViolation { .. } => Some(
            "inspect the decoded source-of-truth rows and wipe/rebuild the pre-production demo DB if the invariant cannot be reconciled"
                .to_string(),
        ),
        DynamicJepaError::DomainPackNotFound { .. } => {
            Some("register the domain pack before running this command".to_string())
        }
        _ => None,
    }
}

fn print_json(value: &Value) -> Result<(), serde_json::Error> {
    let text = serde_json::to_string_pretty(value)?;
    println!("{text}");
    Ok(())
}

fn now_unix_ms() -> DynamicJepaResult<i64> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| {
            DynamicJepaError::validation(
                "system_clock",
                format!("system clock is before Unix epoch: {err}"),
                "fix host clock before writing timestamped DynamicJEPA records",
            )
        })?;
    Ok(duration.as_millis() as i64)
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher.finalize().into()
}

fn hex_bytes(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn toml_validation_error(field: impl Into<String>, message: impl Into<String>) -> DynamicJepaError {
    DynamicJepaError::validation(
        field,
        message,
        "fix the strict domain pack TOML source of truth; Phase 3 registration does not substitute defaults",
    )
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DomainPackToml {
    domain: DomainToml,
    time: TimeToml,
    state: SchemaToml,
    action: SchemaToml,
    outcome: SchemaToml,
    entity: SchemaToml,
    instruments: Vec<InstrumentToml>,
    adapters: Vec<AdapterToml>,
    objectives: Vec<ObjectiveToml>,
    invariants: Vec<InvariantToml>,
    dataset_policy: DatasetPolicyToml,
    planner_policy: PlannerPolicyToml,
    verification_policy: VerificationPolicyToml,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DomainToml {
    id: String,
    version: String,
    title: String,
    schema_version: u8,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TimeToml {
    field: FieldToml,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SchemaToml {
    fields: Vec<FieldToml>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FieldToml {
    name: String,
    kind: TomlKind,
    required: bool,
    min: Option<f64>,
    max: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum TomlKind {
    Name(String),
    Table(KindTableToml),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct KindTableToml {
    #[serde(rename = "type")]
    kind_type: String,
    variants: Option<Vec<String>>,
    dim: Option<usize>,
    encoding: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct InstrumentToml {
    id: String,
    kind: TomlKind,
    input_fields: Vec<String>,
    output_shape: Vec<usize>,
    normalization: NormalizationToml,
    required: bool,
    model_ref: Option<String>,
    version: u8,
    validation: InstrumentValidationToml,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct NormalizationToml {
    #[serde(rename = "type")]
    norm_type: String,
    mean: Option<f64>,
    std: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct InstrumentValidationToml {
    require_finite: bool,
    min: Option<f64>,
    max: Option<f64>,
    reject_nan: bool,
    reject_inf: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AdapterToml {
    id: String,
    kind: String,
    version: u8,
    mapping: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ObjectiveToml {
    id: String,
    kind: String,
    input_panel: String,
    target: String,
    loss_weight: f64,
    required_dataset_fields: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct InvariantToml {
    id: String,
    expression: String,
    severity: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DatasetPolicyToml {
    default_split: String,
    split_key: Option<String>,
    split_buckets: Option<BTreeMap<String, f64>>,
    default_objective: String,
    negative_sampling: String,
    min_negatives_per_row: u32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PlannerPolicyToml {
    candidate_actions: CandidateActionsToml,
    guards: Vec<String>,
    surprise_threshold: f32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CandidateActionsToml {
    kind: String,
    deltas: Option<Vec<i64>>,
    moves: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct VerificationPolicyToml {
    required_cfs: Vec<String>,
    expected_event_count: Option<u64>,
    expected_panel_count: Option<u64>,
    expected_dataset_row_count: Option<u64>,
    expected_trajectory_count: Option<u64>,
    expected_train_rows_min: Option<u64>,
    expected_train_rows_max: Option<u64>,
    expected_val_rows_min: Option<u64>,
    expected_val_rows_max: Option<u64>,
    expected_test_rows_min: Option<u64>,
    expected_test_rows_max: Option<u64>,
}
