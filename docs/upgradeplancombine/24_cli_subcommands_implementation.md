# 24 — CLI Subcommands Implementation Plan

This document is the file-level diff plan for the six new CLI
subcommands (PRs 6, 7, 8, 9, 12, 14 of
`20_implementation_plan_overview.md`) plus the seventh (audit, in PR
14).

## 1. The CLI entry point

`crates/context-graph-cli/src/commands/dynamicjepa.rs`

The existing `enum DynamicJepaCommands` (29 variants) is extended
with seven new variants:

```rust
pub enum DynamicJepaCommands {
    // ... existing 29 variants ...

    /// Build a frozen constellation centroid set for (domain, subject).
    BuildConstellation(BuildConstellationArgs),

    /// List all constellations in the bundle's RocksDB.
    ListConstellations(ListConstellationsArgs),

    /// Read a single constellation centroid by (domain, subject, modality).
    GetConstellation(GetConstellationArgs),

    /// Calibrate the per-modality threshold tau using same-session-band 10th-percentile.
    CalibrateThreshold(CalibrateThresholdArgs),

    /// Re-calibrate, superseding an existing threshold row with a new one.
    ReCalibrateThreshold(ReCalibrateThresholdArgs),

    /// Run the pairwise mutual-information audit (KSG default; MINE optional).
    AuditPairwiseMi(AuditPairwiseMiArgs),

    /// Aggregate audit-log signal_yield to compute the bundle's MC ratio.
    ComputeMcRatio(ComputeMcRatioArgs),
}

impl DynamicJepaCommands {
    fn operation(&self) -> &'static str {
        match self {
            // ... existing 29 ...
            Self::BuildConstellation(_)    => "dynamicjepa_build_constellation",
            Self::ListConstellations(_)    => "dynamicjepa_list_constellations",
            Self::GetConstellation(_)      => "dynamicjepa_get_constellation",
            Self::CalibrateThreshold(_)    => "dynamicjepa_calibrate_threshold",
            Self::ReCalibrateThreshold(_)  => "dynamicjepa_recalibrate_threshold",
            Self::AuditPairwiseMi(_)       => "dynamicjepa_audit_pairwise_mi",
            Self::ComputeMcRatio(_)        => "dynamicjepa_compute_mc_ratio",
        }
    }
}
```

## 2. Args struct definitions

### 2.1 `BuildConstellationArgs` (PR 7)

```rust
#[derive(Args, Debug)]
pub struct BuildConstellationArgs {
    /// Path to the RocksDB data directory.
    #[arg(long, env = "CONTEXT_GRAPH_STORAGE_PATH")]
    pub db: PathBuf,

    /// Domain pack id, e.g. "career_taxonomy".
    #[arg(long)]
    pub domain: String,

    /// Domain pack semver.
    #[arg(long = "domain-version")]
    pub domain_version: String,

    /// Subject id; "global" or "<field>=<value>" (e.g. "cluster.software").
    #[arg(long)]
    pub subject: String,

    /// Optional minimum LOO stability override.
    #[arg(long = "require-loo-min")]
    pub require_loo_min: Option<f32>,

    /// Allow centroids with LOO below threshold to be persisted with
    /// a provenance flag instead of typed-error.
    #[arg(long = "allow-low-loo")]
    pub allow_low_loo: bool,

    /// Optional override for the source-event selector
    /// (default: TOML-declared selector for the subject).
    #[arg(long = "source-event-selector")]
    pub source_event_selector: Option<String>,

    /// Emit machine-readable JSON to stdout.
    #[arg(long)]
    pub json: bool,
}
```

### 2.2 `ListConstellationsArgs`, `GetConstellationArgs` (PR 7)

```rust
#[derive(Args, Debug)]
pub struct ListConstellationsArgs {
    #[arg(long, env = "CONTEXT_GRAPH_STORAGE_PATH")]
    pub db: PathBuf,

    /// Filter by domain id.
    #[arg(long)]
    pub domain: Option<String>,

    /// Filter by subject id.
    #[arg(long)]
    pub subject: Option<String>,

    #[arg(long, default_value_t = 100)]
    pub limit: usize,

    #[arg(long, default_value_t = 0)]
    pub offset: usize,

    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct GetConstellationArgs {
    #[arg(long, env = "CONTEXT_GRAPH_STORAGE_PATH")]
    pub db: PathBuf,

    #[arg(long)]
    pub domain: String,

    #[arg(long)]
    pub subject: String,

    #[arg(long)]
    pub modality: u32,

    /// Include the full centroid vector in the output (default: head + tail only).
    #[arg(long = "include-vector")]
    pub include_vector: bool,

    #[arg(long)]
    pub json: bool,
}
```

### 2.3 `CalibrateThresholdArgs`, `ReCalibrateThresholdArgs` (PR 8)

```rust
#[derive(Args, Debug)]
pub struct CalibrateThresholdArgs {
    #[arg(long, env = "CONTEXT_GRAPH_STORAGE_PATH")]
    pub db: PathBuf,

    #[arg(long)]
    pub domain: String,

    #[arg(long)]
    pub subject: String,

    #[arg(long)]
    pub modality: u32,

    /// Selector for the disjoint calibration set (e.g. "step_index>=900").
    #[arg(long = "calibration-set-selector")]
    pub calibration_set_selector: String,

    /// Percentile for tau (default 10th-percentile floor).
    #[arg(long, default_value_t = 10)]
    pub percentile: u8,

    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct ReCalibrateThresholdArgs {
    #[arg(long, env = "CONTEXT_GRAPH_STORAGE_PATH")]
    pub db: PathBuf,

    #[arg(long)]
    pub domain: String,

    #[arg(long)]
    pub subject: String,

    #[arg(long)]
    pub modality: u32,

    /// UUID of the threshold-calibration row being superseded.
    #[arg(long = "supersedes-uuid")]
    pub supersedes_uuid: String,

    #[arg(long = "calibration-set-selector")]
    pub calibration_set_selector: String,

    #[arg(long, default_value_t = 10)]
    pub percentile: u8,

    /// Required reason text recorded in the audit row.
    #[arg(long)]
    pub reason: String,

    #[arg(long)]
    pub json: bool,
}
```

### 2.4 `AuditPairwiseMiArgs` (PR 14)

```rust
#[derive(Args, Debug)]
pub struct AuditPairwiseMiArgs {
    #[arg(long, env = "CONTEXT_GRAPH_STORAGE_PATH")]
    pub db: PathBuf,

    #[arg(long)]
    pub domain: String,

    /// Number of events to sample for the audit (default 1000).
    #[arg(long = "sample-size", default_value_t = 1000)]
    pub sample_size: usize,

    /// Estimator: ksg (default) or mine.
    #[arg(long, default_value = "ksg")]
    pub estimator: String,

    /// KSG nearest-neighbour k.
    #[arg(long = "ksg-k", default_value_t = 5)]
    pub ksg_k: usize,

    /// MINE training iterations.
    #[arg(long = "mine-iters", default_value_t = 5000)]
    pub mine_iters: usize,

    /// Bootstrap seed.
    #[arg(long, default_value_t = 20260501u64)]
    pub seed: u64,

    /// Output directory.
    #[arg(long = "output-dir")]
    pub output_dir: PathBuf,

    #[arg(long)]
    pub json: bool,
}
```

### 2.5 `ComputeMcRatioArgs` (PR 12)

```rust
#[derive(Args, Debug)]
pub struct ComputeMcRatioArgs {
    #[arg(long, env = "CONTEXT_GRAPH_STORAGE_PATH")]
    pub db: PathBuf,

    #[arg(long)]
    pub domain: String,

    /// Optional bundle root path (alternative to --db; if set, computes
    /// MC ratio from the bundle's bundled RocksDB instead).
    #[arg(long = "bundle-root", conflicts_with = "db")]
    pub bundle_root: Option<PathBuf>,

    #[arg(long = "output-dir")]
    pub output_dir: PathBuf,

    #[arg(long)]
    pub json: bool,
}
```

## 3. Subcommand dispatch

Every new variant is dispatched in `run_dynamicjepa()`:

```rust
pub fn run_dynamicjepa(args: DynamicJepaArgs) -> Result<(), CliError> {
    // existing schema-version pre-check (PR 1)
    validate_bundle_schema_version(&args.bundle_root, 2)?;

    match args.command {
        // ... existing 29 dispatches ...

        DynamicJepaCommands::BuildConstellation(a)   => run_build_constellation(a),
        DynamicJepaCommands::ListConstellations(a)   => run_list_constellations(a),
        DynamicJepaCommands::GetConstellation(a)     => run_get_constellation(a),
        DynamicJepaCommands::CalibrateThreshold(a)   => run_calibrate_threshold(a),
        DynamicJepaCommands::ReCalibrateThreshold(a) => run_recalibrate_threshold(a),
        DynamicJepaCommands::AuditPairwiseMi(a)      => run_audit_pairwise_mi(a),
        DynamicJepaCommands::ComputeMcRatio(a)       => run_compute_mc_ratio(a),
    }
}
```

## 4. Per-subcommand implementation files

| Command | Implementation file | LoC est. |
| --- | --- | ---: |
| `build-constellation` | `src/commands/build_constellation.rs` | ~350 |
| `list-constellations` | `src/commands/list_constellations.rs` | ~120 |
| `get-constellation` | `src/commands/get_constellation.rs` | ~80 |
| `calibrate-threshold` | `src/commands/calibrate_threshold.rs` | ~250 |
| `recalibrate-threshold` | `src/commands/recalibrate_threshold.rs` | ~200 |
| `audit-pairwise-mi` | `src/commands/audit_pairwise_mi.rs` | ~400 (+ KSG impl in `mi_audit/`) |
| `compute-mc-ratio` | `src/commands/mc_ratio.rs` | ~250 |

## 5. `build-constellation` implementation skeleton (PR 7)

```rust
// src/commands/build_constellation.rs

use context_graph_storage::dynamicjepa::{
    constellation_store, get_domain_pack, list_normalized_states,
    list_instrument_readings_for_event,
};
use context_graph_core::dynamicjepa::{
    ConstellationCentroid, ConstellationId, DjAuditRecord,
};

pub fn run_build_constellation(args: BuildConstellationArgs)
    -> Result<(), CliError>
{
    let store = open_db_writable(&args.db)?;
    let domain_pack = get_domain_pack(&store, &args.domain, &args.domain_version)?
        .ok_or(CliError::DomainPackNotRegistered)?;

    // 1. Resolve the source-event selector
    let selector = match (&args.source_event_selector, &args.subject.as_str()) {
        (Some(s), _) => parse_selector(s)?,
        (None, "global") => SourceEventSelector::All,
        (None, subject_id) => domain_pack
            .constellation_subjects.iter()
            .find(|s| s.id == subject_id)
            .map(|s| SourceEventSelector::Field(s.field.clone(), s.value.clone()))
            .ok_or(CliError::ConstellationSubjectFieldUndeclared)?,
    };

    // 2. Determine target modalities
    let modalities = match &args.subject.as_str() {
        "global" => domain_pack.constellation_global.as_ref()
            .map(|c| c.modalities.clone())
            .unwrap_or(ConstellationModalities::All),
        subject_id => domain_pack.constellation_subjects.iter()
            .find(|s| s.id == subject_id)
            .map(|s| s.modalities.clone())
            .ok_or(CliError::ConstellationSubjectFieldUndeclared)?,
    };
    let modality_ids = match &modalities {
        ConstellationModalities::All => domain_pack.declared_instruments
            .iter().map(|i| i.id).collect::<Vec<_>>(),
        ConstellationModalities::List(v) => v.clone(),
    };

    // 3. Read all events matching the selector
    let states = list_normalized_states_matching(&store, &domain_pack, &selector)?;
    if states.len() < 5 {
        return Err(CliError::ConstellationReferenceSetTooSmall {
            count: states.len(),
        });
    }

    // 4. Per-modality centroid construction
    let mut output_per_modality = Vec::new();
    let mut batch = WriteBatch::default();

    for modality_id in modality_ids {
        let readings: Vec<_> = states.iter()
            .filter_map(|s| {
                list_instrument_readings_for_event(&store, &s.event_id)
                    .ok()
                    .and_then(|r| r.into_iter()
                        .find(|read| read.instrument_id == modality_id))
            })
            .collect();

        // Phase-1 protocol from doc 04 §3
        let kept: Vec<&InstrumentReading> = readings.iter()
            .filter(|r| r.vector_norm() >= 1e-6)
            .collect();
        let dropped_zero_norm = readings.len() - kept.len();

        if kept.is_empty() {
            return Err(CliError::ConstellationZeroNormMajority);
        }

        let normalized: Vec<Vec<f32>> = kept.iter()
            .map(|r| l2_normalize(&r.vector))
            .collect();
        let mean = mean_vector(&normalized);
        let centroid = l2_normalize(&mean);

        let loo_stability = compute_loo_stability(&normalized, &centroid)?;
        let require_min = args.require_loo_min.unwrap_or_else(|| {
            domain_pack_loo_min(&domain_pack, &args.subject)
        });
        if loo_stability < require_min && !args.allow_low_loo {
            return Err(CliError::ConstellationLooBelowThreshold {
                modality_id,
                loo_stability,
                threshold: require_min,
            });
        }

        let centroid_record = ConstellationCentroid {
            constellation_id: ConstellationId::new_v4(),
            domain_pack_id: domain_pack.id,
            subject_id: args.subject.clone(),
            modality_id,
            centroid,
            instrument_artifact_hash: domain_pack
                .declared_instruments.iter()
                .find(|i| i.id == modality_id)
                .map(|i| i.artifact_hash)
                .unwrap_or([0u8; 32]),
            reference_set_count: states.len() as u32,
            kept_count: kept.len() as u32,
            dropped_zero_norm: dropped_zero_norm as u32,
            loo_stability,
            calibration_percentile: domain_pack_calibration_percentile(&domain_pack, &args.subject),
            calibration_set_size: domain_pack_calibration_set_size(&domain_pack, &args.subject),
            built_at_unix_ms: now_unix_ms(),
            built_by_run_id: env_run_id(),
        };

        // Persist
        let audit = DjAuditRecord {
            audit_id: AuditId::new_v4(),
            operation: "build_constellation".to_string(),
            // ... other fields ...
            signal_yield: signal_yield_for_operation(
                "build_constellation",
                &DomainPackInfo::from_domain_pack(&domain_pack),
            ),
        };

        constellation_store::put_constellation_with_audit_batch(
            &store,
            &mut batch,
            &centroid_record,
            &audit,
        )?;

        output_per_modality.push(centroid_record);
    }

    store.db().write(batch)?;

    // FSV-10 immediate post-write check
    fsv10_constellation_integrity(&store, &output_per_modality)?;

    if args.json {
        let json = serde_json::to_string_pretty(&BuildConstellationOutput {
            status: "ok".to_string(),
            constellation_uuid: output_per_modality[0].constellation_id.to_string(),
            domain_pack_id: domain_pack.id.to_string(),
            subject_id: args.subject,
            modalities: output_per_modality
                .iter()
                .map(|c| ModalityOutput {
                    modality_id: c.modality_id.0,
                    centroid_head: c.centroid[..8.min(c.centroid.len())].to_vec(),
                    centroid_tail: c.centroid.iter().rev().take(8).cloned().rev().collect(),
                    kept_count: c.kept_count,
                    dropped_zero_norm: c.dropped_zero_norm,
                    loo_stability: c.loo_stability,
                    instrument_artifact_hash: hex::encode(&c.instrument_artifact_hash),
                })
                .collect(),
            audit_id: AuditId::new_v4().to_string(),
        })?;
        println!("{}", json);
    } else {
        // human-readable output ...
    }

    Ok(())
}
```

## 6. `list-constellations` skeleton

```rust
// src/commands/list_constellations.rs

pub fn run_list_constellations(args: ListConstellationsArgs)
    -> Result<(), CliError>
{
    let store = open_db_read_only(&args.db)?;
    let filter = ConstellationFilter {
        domain_uuid: args.domain.as_ref().and_then(|d| {
            get_domain_pack_id(&store, d).ok().flatten()
        }),
        subject_id: args.subject,
        modality_id: None,
    };
    let centroids = list_constellations(&store, filter, args.limit, args.offset)?;

    if args.json {
        let json = serde_json::to_string_pretty(&ListConstellationsOutput {
            count: centroids.len(),
            constellations: centroids.into_iter()
                .map(|c| ConstellationSummary {
                    constellation_id: c.constellation_id.to_string(),
                    domain_pack_id: c.domain_pack_id.to_string(),
                    subject_id: c.subject_id,
                    modality_id: c.modality_id.0,
                    n_dim: c.centroid.len(),
                    loo_stability: c.loo_stability,
                    built_at_unix_ms: c.built_at_unix_ms,
                })
                .collect(),
        })?;
        println!("{}", json);
    }

    Ok(())
}
```

## 7. `calibrate-threshold` skeleton

```rust
// src/commands/calibrate_threshold.rs

pub fn run_calibrate_threshold(args: CalibrateThresholdArgs)
    -> Result<(), CliError>
{
    let store = open_db_writable(&args.db)?;
    let domain_pack = get_domain_pack(&store, &args.domain)?
        .ok_or(CliError::DomainPackNotRegistered)?;

    let centroid = constellation_store::get_constellation(
        &store,
        &domain_pack.id,
        &args.subject,
        InstrumentId(args.modality),
    )?.ok_or(CliError::ConstellationNotFound)?;

    let calibration_selector = parse_selector(&args.calibration_set_selector)?;
    let calibration_set = list_normalized_states_matching(
        &store, &domain_pack, &calibration_selector,
    )?;

    if calibration_set.len() < 50 {
        return Err(CliError::CalibrationSetTooSmall {
            count: calibration_set.len(),
        });
    }

    // Disjoint check (FSV-11 prerequisite)
    let reference_event_set = events_used_to_build_centroid(&store, &centroid)?;
    let calibration_event_set: HashSet<_> = calibration_set.iter()
        .map(|s| s.event_id).collect();
    let overlap: Vec<_> = reference_event_set.intersection(&calibration_event_set).collect();
    if !overlap.is_empty() {
        return Err(CliError::CalibrationSetNotDisjoint {
            overlap_count: overlap.len(),
        });
    }

    // Compute cosines
    let cosines: Vec<f32> = calibration_set.iter()
        .filter_map(|s| {
            list_instrument_readings_for_event(&store, &s.event_id)
                .ok()
                .and_then(|r| r.into_iter()
                    .find(|read| read.instrument_id == InstrumentId(args.modality)))
                .map(|reading| {
                    cosine_similarity(&reading.vector, &centroid.centroid)
                })
        })
        .collect();

    if cosines.is_empty() {
        return Err(CliError::CalibrationSetReadingsMissing);
    }

    // Compute percentile
    let mut sorted = cosines.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
    let idx = ((args.percentile as f64 / 100.0) * sorted.len() as f64) as usize;
    let tau = sorted[idx];

    let calibration = ThresholdCalibration {
        calibration_id: ThresholdCalibrationId::new_v4(),
        domain_pack_id: domain_pack.id,
        subject_id: args.subject.clone(),
        modality_id: InstrumentId(args.modality),
        tau,
        percentile: args.percentile,
        calibration_set_count: cosines.len() as u32,
        calibration_event_uuids_sample: calibration_set.iter()
            .take(16)
            .map(|s| s.event_id)
            .collect(),
        calibration_set_disjoint_proof: true,
        calibration_min: sorted.first().copied().unwrap_or(0.0),
        calibration_max: sorted.last().copied().unwrap_or(0.0),
        calibration_p10: sorted[(0.10 * sorted.len() as f64) as usize.min(sorted.len() - 1)],
        supersede_seq: 0,
        supersedes_uuid: None,
        reason: None,
        calibrated_at_unix_ms: now_unix_ms(),
    };

    // Persist
    let mut batch = WriteBatch::default();
    let audit = DjAuditRecord {
        // ... fields ...
        operation: "calibrate_threshold".to_string(),
        signal_yield: signal_yield_for_operation("calibrate_threshold", &DomainPackInfo::from_domain_pack(&domain_pack)),
    };
    threshold_store::put_threshold_calibration_with_audit_batch(
        &store, &mut batch, &calibration, &audit,
    )?;
    store.db().write(batch)?;

    // Emit
    if args.json {
        // emit JSON
    }

    Ok(())
}
```

## 8. `compute-mc-ratio` (per `23_audit_log_implementation.md` §1.2)

Already specified there; brief note here: `mc_ratio.rs` is shared
between the CLI command and the `research-smoke` orchestrator. The
orchestrator invokes `compute_mc_ratio_from_paths(...)` directly,
not via `Command`.

## 9. Help-text discipline

Each new subcommand's `--help` text follows the existing v1 pattern,
naming:

- The operation kind verbatim.
- The CFs the subcommand reads / writes.
- The FSV harness that audits its output.
- Typed errors raised on edge cases.

Example for `build-constellation`:

```
Build a frozen constellation centroid set for (domain, subject).
Implements TCT Phase 1 from PAPER.md §5.2.

Operation kind: dynamicjepa_build_constellation
Reads: dj_normalized_states, dj_instrument_readings, dj_instrument_registry
Writes: dj_constellations, dj_audit_log
Verified by: FSV-10 (constellation integrity)

Edge cases:
  --domain unregistered     : DomainPackNotRegistered
  --subject undeclared      : ConstellationSubjectFieldUndeclared
  reference set < 5         : ConstellationReferenceSetTooSmall
  zero-norm majority        : ConstellationZeroNormMajority
  LOO < require_loo_min     : ConstellationLooBelowThreshold
  artifact hash drift       : ConstellationInstrumentHashMismatch
```

## 10. MCP tool wiring

`crates/context-graph-mcp/src/handlers/tools/dynamicjepa_tools.rs`

Each new CLI subcommand has a 1:1 MCP tool definition. The pattern
follows the existing `dj_register_domain` etc.\ tools:

```rust
// Tool input schemas
const BUILD_CONSTELLATION_TOOL_NAME: &str = "dj_build_constellation";

const BUILD_CONSTELLATION_INPUT_SCHEMA: &str = r#"{
  "type": "object",
  "additionalProperties": false,
  "required": ["db", "domain", "domain_version", "subject"],
  "properties": {
    "db": { "type": "string" },
    "domain": { "type": "string" },
    "domain_version": { "type": "string" },
    "subject": { "type": "string" },
    "require_loo_min": { "type": "number", "minimum": 0.0, "maximum": 1.0 },
    "allow_low_loo": { "type": "boolean" },
    "source_event_selector": { "type": "string" }
  }
}"#;

pub async fn handle_build_constellation_tool(
    request: ToolCallRequest,
) -> Result<ToolCallResponse, ToolError> {
    let args: BuildConstellationArgs = serde_json::from_value(request.arguments)?;
    let output = run_build_constellation_async(args).await?;
    Ok(ToolCallResponse::from_json(output))
}
```

`additionalProperties: false` is required per the V1 lesson
("MCP schema enforcement means handler params NOT in schema are
silently dropped"). Each new tool's schema is registered in the MCP
server's tool list.

## 11. Test additions per PR

Each new subcommand has a per-command integration test file:

| PR | Test file | Tests |
| --- | --- | ---: |
| 7 | `tests/cli_build_constellation.rs` | 8 |
| 7 | `tests/cli_list_get_constellations.rs` | 4 |
| 8 | `tests/cli_calibrate_threshold.rs` | 7 |
| 8 | `tests/cli_recalibrate_threshold.rs` | 5 |
| 12 | `tests/cli_compute_mc_ratio.rs` | 6 |
| 14 | `tests/cli_audit_pairwise_mi.rs` | 9 |

Each integration test uses the real CLI binary (per the v1 pattern
in `tests/dynamicjepa_research_smoke_fsv.rs`) and verifies edge
cases produce the documented typed errors without mutating the
filesystem.

## 12. Wall-time budget per command

| Command | Est.\ wall time on RTX 5090 |
| --- | ---: |
| `build-constellation` (50 modalities for career-A1) | ~2 s |
| `list-constellations` | < 100 ms |
| `get-constellation` | < 50 ms |
| `calibrate-threshold` (per modality) | < 200 ms |
| `recalibrate-threshold` | < 200 ms |
| `audit-pairwise-mi` (1{,}000 events × 45 pairs) | ~80 s |
| `compute-mc-ratio` | < 300 ms |

These fit comfortably within the existing tier ladder budgets.
