//! DynamicJEPA CLI phase 3: domain-pack registry and physical inspection.
//!
//! These commands intentionally stop at Phase 3 scope. Mutating commands write
//! to the real RocksDB DynamicJEPA column families, then immediately read the
//! source-of-truth rows back through separate storage inspection paths.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use clap::{Args, Subcommand};
use context_graph_core::dynamicjepa::{
    ActionSchema, AdapterId, AdapterSpec, CandidateActionConfig, DatasetPolicy, DjRecordHeader,
    DomainPack, DomainPackId, DynamicJepaError, DynamicJepaRecord, DynamicJepaResult, EntitySchema,
    FieldKind, FieldSpec, InstrumentId, InstrumentKind, InstrumentSpec, InstrumentValidation,
    InvariantSpec, Normalization, ObjectiveSpec, OutcomeSchema, PlannerPolicy, StateSchema,
    TimeEncoding, TimeSchema, VerificationPolicy, DOMAIN_PACK_RECORD_VERSION,
};
use context_graph_storage::dynamicjepa::column_families::{
    CF_DJ_ADAPTER_REGISTRY, CF_DJ_AUDIT_LOG, CF_DJ_DOMAIN_PACKS,
    CF_DJ_DOMAIN_PACK_BY_NAME_VERSION, CF_DJ_INSTRUMENT_REGISTRY, DJ_CF_NAMES,
};
use context_graph_storage::dynamicjepa::keys::{
    adapter_registry_key, instrument_registry_key, name_version_key, uuid_key,
};
use context_graph_storage::dynamicjepa::{
    get_adapter_spec, get_domain_pack, get_domain_pack_by_storage_id, get_instrument_spec,
    inspect_cf, list_domain_packs, put_domain_pack, snapshot_dj_counts,
};
use context_graph_storage::teleological::{RocksDbTeleologicalStore, TeleologicalStoreConfig};
use serde::Deserialize;
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
}

impl DynamicJepaCommands {
    fn operation(&self) -> &'static str {
        match self {
            Self::RegisterDomain(_) => "dynamicjepa_register_domain",
            Self::ListDomains(_) => "dynamicjepa_list_domains",
            Self::GetDomain(_) => "dynamicjepa_get_domain",
            Self::InspectCounts(_) => "dynamicjepa_inspect_counts",
            Self::InspectCf(_) => "dynamicjepa_inspect_cf",
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
    #[arg(long, default_value = "1.0.0")]
    pub version: String,

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
                info!(operation, status = "ok", "DynamicJEPA CLI command completed");
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
        let by_name_version =
            get_domain_pack(db, &domain_pack.id, &domain_pack.version)?.ok_or_else(|| {
                DynamicJepaError::SourceOfTruthMissing {
                    cf: CF_DJ_DOMAIN_PACK_BY_NAME_VERSION.to_string(),
                    key: name_version_key(&domain_pack.id, &domain_pack.version).to_vec(),
                }
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
            let stored = get_adapter_spec(db, domain_pack_uuid, &spec.adapter_id)?.ok_or_else(
                || DynamicJepaError::SourceOfTruthMissing {
                    cf: CF_DJ_ADAPTER_REGISTRY.to_string(),
                    key: adapter_registry_key(domain_pack_uuid, &spec.adapter_id).to_vec(),
                },
            )?;
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
                "count_delta": count_delta(&before_counts, &after_counts),
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
    let record = get_domain_pack(db, &id, &args.version)?;
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
                    "version": args.version,
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

fn candidate_actions_from_toml(value: CandidateActionsToml) -> DynamicJepaResult<CandidateActionConfig> {
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
) -> BTreeMap<String, i64> {
    let mut delta = BTreeMap::new();
    for (cf, before_count) in before {
        let after_count = after.get(cf).copied().unwrap_or(0);
        delta.insert(cf.clone(), after_count as i64 - *before_count as i64);
    }
    for (cf, after_count) in after {
        if !before.contains_key(cf) {
            delta.insert(cf.clone(), *after_count as i64);
        }
    }
    delta
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
    let duration = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|err| {
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

fn toml_validation_error(
    field: impl Into<String>,
    message: impl Into<String>,
) -> DynamicJepaError {
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
