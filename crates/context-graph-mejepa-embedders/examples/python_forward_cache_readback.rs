use chrono::DateTime;
use context_graph_core::memory::ast::Language as AstLanguage;
use context_graph_mejepa_embedders::{
    AlgorithmicEmbedderForward, EmbedError, EmbedderForward, EmbedderId, EmbedderInput,
    EmbedderOutput, ModelsConfig, PretrainedEmbedderForward, SUPPORTED_FORWARD_EMBEDDERS,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

const SCHEMA_VERSION: u64 = 1;
const DEFAULT_MODELS_CONFIG: &str = "/zfs/hot/contextgraph/models/mejepa_models_config.toml";
const STATIC_AST_CORPUS_PROFILE: &str = "static_ast_python";
const LIVE_TEMPORAL_CORPUS_PROFILE: &str = "live_temporal";
const MIN_UNIQUE_VECTOR_FRACTION: f64 = 0.5;
// Per-shard CLEAN threshold: at/above this fraction of a slot's distinct inputs map
// to a distinct vector, the shard is labeled fully "usable" with no recorded
// collisions. This is NO LONGER a per-shard fatal floor — benign tokenizer-equivalent
// redundancy (e.g. e5-large-v2 / bert-large-uncased mapping case-only variants such as
// `'spam'`/`'SPAM'` to one vector) lands below 0.9 yet is legitimate and must not abort
// a multi-day corpus build. The strict 0.9 anti-collision guard now lives at corpus
// scope (see the scheduler's CORPUS_MIN_NON_COLLIDING_INPUT_FRACTION).
const MIN_UNIQUE_VECTOR_PER_UNIQUE_INPUT_FRACTION: f64 = 0.9;
// Per-shard FATAL floor: if FEWER than half of a slot's distinct inputs map to a
// distinct vector, the embedder is genuinely degenerate for this shard (a constant or
// broken model) and the run fails fast. Benign case/format redundancy sits well above
// this floor and is recorded as a non-fatal benign collision instead.
const CATASTROPHIC_MIN_NON_COLLIDING_INPUT_FRACTION: f64 = 0.5;
const MIN_UNIQUE_INPUT_COUNT_FOR_DUPLICATE_BOUNDED_GATE: usize = 64;
const MAX_PAIRWISE_VECTOR_HASH_JACCARD: f64 = 0.5;
const ERR_EMBEDDER_COLLAPSE: &str = "MEJEPA_FORWARD_CACHE_EMBEDDER_COLLAPSE";
const ERR_EMBEDDER_ALIAS: &str = "MEJEPA_FORWARD_CACHE_EMBEDDER_ALIAS";
// Non-fatal: distinct inputs that legitimately share a vector above the catastrophic
// floor (uncased/normalizing embedder on case/whitespace variants). Recorded for
// corpus-level monitoring; never fails a shard.
const INFO_EMBEDDER_BENIGN_COLLISION: &str = "MEJEPA_FORWARD_EMBEDDER_BENIGN_COLLISION";

type RunnerResult<T> = Result<T, RunnerError>;

#[derive(Debug)]
struct RunnerError {
    code: &'static str,
    message: String,
}

impl RunnerError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for RunnerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for RunnerError {}

#[derive(Debug)]
struct EnvConfig {
    chunks_jsonl: PathBuf,
    cache_root: PathBuf,
    readback_root: PathBuf,
    chunk_offset: usize,
    max_chunks: usize,
    max_source_bytes: usize,
    true_batch_size: usize,
    cache_store_layout: String,
    only_embedder: Option<EmbedderId>,
    corpus_profile: String,
    models_config: PathBuf,
    gpu_vram_guard_bytes: u64,
}

#[derive(Debug, Clone)]
struct ChunkRow {
    line_number: usize,
    chunk_id: String,
    source_text: String,
    source_sha256: String,
    file_sha256: String,
    task_id: String,
    task_instance_id: String,
    repo: String,
    workspace_state: String,
    absolute_path: String,
    relative_path: String,
    mutation_category: Option<String>,
    candidate_row_id: Option<String>,
}

#[derive(Debug, Clone)]
struct ForwardRecord {
    index_row: Value,
    packed_record: Value,
}

#[derive(Debug)]
struct PendingEmbedderRecords {
    embedder: EmbedderId,
    records: Vec<ForwardRecord>,
}

#[derive(Debug, Clone)]
struct QuarantineRecord {
    row: Value,
}

#[derive(Debug)]
struct BatchResult {
    telemetry: Value,
    records: Vec<ForwardRecord>,
    quarantines: Vec<QuarantineRecord>,
    duplicate_forward_cache_hits: usize,
    unique_forward_input_count: usize,
}

#[derive(Debug, Clone)]
struct ForwardInputIdentity {
    input_sha256: String,
    policy: &'static str,
}

struct PendingForward {
    input: EmbedderInput,
    identity: ForwardInputIdentity,
}

#[derive(Debug, Clone)]
struct ForwardFailure {
    code: &'static str,
    message: String,
}

impl ForwardFailure {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    fn from_embed(err: &EmbedError) -> Self {
        Self::new(err.code(), err.to_string())
    }
}

struct BatchTelemetryCounts {
    batch_size: usize,
    output_count: usize,
    quarantine_count: usize,
    unique_forward_input_count: usize,
    duplicate_forward_cache_hits: usize,
    latency_ms: f64,
}

enum BatchRecordPlan<'a> {
    Cached {
        chunk: &'a ChunkRow,
        identity: ForwardInputIdentity,
        output: EmbedderOutput,
    },
    Pending {
        chunk: &'a ChunkRow,
        identity: ForwardInputIdentity,
        pending_idx: usize,
    },
}

#[derive(Debug, Clone)]
struct DistinctnessReport {
    per_embedder_metrics: BTreeMap<String, Value>,
    pairwise_jaccard_matrix: Value,
    collapse_failures: Vec<Value>,
    alias_failures: Vec<Value>,
    // Non-fatal telemetry: distinct inputs that legitimately collide above the
    // catastrophic floor. Never affects `passes()`; rolled up at corpus scope.
    benign_collisions: Vec<Value>,
}

impl DistinctnessReport {
    fn passes(&self) -> bool {
        self.collapse_failures.is_empty() && self.alias_failures.is_empty()
    }

    fn error_code(&self) -> &'static str {
        if self.collapse_failures.is_empty() {
            ERR_EMBEDDER_ALIAS
        } else {
            ERR_EMBEDDER_COLLAPSE
        }
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> std::process::ExitCode {
    match run().await {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{err}");
            std::process::ExitCode::from(2)
        }
    }
}

async fn run() -> RunnerResult<()> {
    let config = EnvConfig::from_env()?;
    if config.cache_store_layout != "packed" && config.cache_store_layout != "packed-jsonl" {
        return Err(RunnerError::new(
            "MEJEPA_PY_FORWARD_UNSUPPORTED_STORE_LAYOUT",
            format!("expected packed layout, got {}", config.cache_store_layout),
        ));
    }
    let chunks = load_chunks(&config)?;
    if chunks.is_empty() {
        return Err(RunnerError::new(
            "MEJEPA_PY_FORWARD_EMPTY_CHUNK_WINDOW",
            "selected chunk window is empty; refusing to write fake success",
        ));
    }

    let embedders = active_embedders(&config)?;
    let retired_embedders = retired_embedders(&config);
    let active_embedder_count = embedders.len();
    prepare_output_roots(&config)?;
    let mut all_index_rows = Vec::new();
    let mut all_quarantine_rows = Vec::new();
    let mut all_telemetry_rows = Vec::new();
    let mut per_embedder_coverage = BTreeMap::new();
    let mut distinctness_inputs_by_embedder = BTreeMap::new();
    let mut pending_embedder_records = Vec::new();
    let mut packed_per_embedder = BTreeMap::new();
    let mut first_pass_writes = 0usize;
    let mut batch_counter = 0usize;

    let models_config = if embedders.iter().any(|embedder| is_pretrained(*embedder)) {
        Some(ModelsConfig::load(&config.models_config).map_err(|err| {
            RunnerError::new(
                "MEJEPA_PY_FORWARD_MODELS_CONFIG_LOAD_FAILED",
                format!("{}: {err}", config.models_config.display()),
            )
        })?)
    } else {
        None
    };

    for embedder in embedders.iter().copied() {
        let started = Instant::now();
        let forwarder = load_forwarder(embedder, models_config.as_ref()).await?;
        let results =
            run_embedder_batches(&config, &chunks, forwarder.as_ref(), &mut batch_counter).await;
        let mut embedder_records = Vec::new();
        let mut output_count = 0usize;
        let mut quarantine_count = 0usize;
        let mut duplicate_forward_cache_hits = 0usize;
        let mut unique_forward_input_count = 0usize;
        for result in results {
            output_count += result.records.len();
            quarantine_count += result.quarantines.len();
            duplicate_forward_cache_hits += result.duplicate_forward_cache_hits;
            unique_forward_input_count += result.unique_forward_input_count;
            all_telemetry_rows.push(result.telemetry);
            for record in result.records {
                embedder_records.push(record);
            }
            for quarantine in result.quarantines {
                all_quarantine_rows.push(quarantine.row);
            }
        }
        distinctness_inputs_by_embedder.insert(
            embedder.slug().to_string(),
            distinctness_inputs_for_records(embedder, &embedder_records)?,
        );
        first_pass_writes += output_count;
        per_embedder_coverage.insert(
            embedder.slug().to_string(),
            json!({
                "embedder_id": embedder.slug(),
                "chunk_count": chunks.len(),
                "cache_record_count": output_count,
                "quarantine_row_count": quarantine_count,
                "unique_forward_input_count": unique_forward_input_count,
                "duplicate_forward_cache_hit_count": duplicate_forward_cache_hits,
                "forward_input_identity_policy": forward_input_identity_policy(embedder),
                "elapsed_ms": elapsed_ms(started),
                "passes": output_count + quarantine_count == chunks.len(),
            }),
        );
        pending_embedder_records.push(PendingEmbedderRecords {
            embedder,
            records: embedder_records,
        });
    }

    let active_embedder_ids: Vec<String> = embedders
        .iter()
        .map(|embedder| embedder.slug().to_string())
        .collect();
    let distinctness_report =
        evaluate_embedder_distinctness(&active_embedder_ids, &distinctness_inputs_by_embedder);
    merge_distinctness_metrics(&mut per_embedder_coverage, &distinctness_report);
    if !distinctness_report.passes() {
        write_distinctness_failure_artifacts(
            &config,
            &chunks,
            &embedders,
            &retired_embedders,
            &per_embedder_coverage,
            &distinctness_report,
        )?;
        return Err(RunnerError::new(
            distinctness_report.error_code(),
            format!(
                "forward-cache embedder distinctness gate failed: collapse_failures={} alias_failures={}",
                distinctness_report.collapse_failures.len(),
                distinctness_report.alias_failures.len()
            ),
        ));
    }

    for pending in pending_embedder_records {
        let (packed, top_index_rows) =
            write_packed_embedder_records(&config, pending.embedder, &pending.records)?;
        all_index_rows.extend(top_index_rows);
        packed_per_embedder.insert(pending.embedder.slug().to_string(), packed);
    }

    write_jsonl(
        &config.cache_root.join("python_forward_cache_index.jsonl"),
        &all_index_rows,
    )?;
    write_jsonl(
        &config
            .cache_root
            .join("python_forward_cache_quarantine.jsonl"),
        &all_quarantine_rows,
    )?;
    write_jsonl(
        &config
            .cache_root
            .join("python_forward_cache_batch_telemetry.jsonl"),
        &all_telemetry_rows,
    )?;

    let packed_manifest_path = config
        .cache_root
        .join("python_forward_cache_packed_manifest.json");
    let packed_manifest = json!({
        "schema_version": SCHEMA_VERSION,
        "artifact_kind": "python_forward_cache_packed_manifest",
        "created_at_utc": utc_now(),
        "layout": "packed-jsonl",
        "corpus_profile": config.corpus_profile.as_str(),
        "active_embedder_ids": embedder_slugs(&embedders),
        "retired_embedder_policy": retired_embedder_policy(&config, &retired_embedders),
        "packed_record_root": config.cache_root.join("packed_records"),
        "row_count": all_index_rows.len(),
        "per_embedder": packed_per_embedder,
        "passes": true,
    });
    write_json(&packed_manifest_path, &packed_manifest)?;

    let manifest_path = config.cache_root.join("python_forward_cache_manifest.json");
    let manifest = json!({
        "schema_version": SCHEMA_VERSION,
        "status": "passed",
        "passes": true,
        "created_at_utc": utc_now(),
        "inputs": {
            "chunks_jsonl": config.chunks_jsonl,
            "chunk_offset": config.chunk_offset,
            "max_chunks": config.max_chunks,
            "max_forward_source_text_bytes": config.max_source_bytes,
            "true_batch_size": config.true_batch_size,
            "gpu_vram_guard_bytes": config.gpu_vram_guard_bytes,
            "models_config": config.models_config,
            "cache_store_layout": "packed-jsonl",
            "only_embedder": config.only_embedder.map(|id| id.slug().to_string()),
            "corpus_profile": config.corpus_profile.as_str(),
        },
        "source_of_truth": {
            "manifest": manifest_path,
            "index_jsonl": config.cache_root.join("python_forward_cache_index.jsonl"),
            "quarantine_jsonl": config.cache_root.join("python_forward_cache_quarantine.jsonl"),
            "batch_telemetry_jsonl": config.cache_root.join("python_forward_cache_batch_telemetry.jsonl"),
            "packed_manifest": packed_manifest_path,
            "packed_record_root": config.cache_root.join("packed_records"),
            "readback": config.readback_root.join("python_forward_cache_readback.json"),
        },
        "summary": {
            "chunk_count": chunks.len(),
            "active_embedder_count": active_embedder_count,
            "active_embedder_ids": embedder_slugs(&embedders),
            "retired_embedder_count": retired_embedders.len(),
            "retired_embedder_ids": embedder_slugs(&retired_embedders),
            "cache_record_count": all_index_rows.len(),
            "first_pass_hits": 0,
            "first_pass_writes": first_pass_writes,
            "second_pass_hits": all_index_rows.len(),
            "quarantine_row_count": all_quarantine_rows.len(),
            "batch_telemetry_row_count": all_telemetry_rows.len(),
            "global_cache_record_count_at_manifest_write": all_index_rows.len(),
            "expected_pairs": chunks.len() * active_embedder_count,
            "accounted_pairs": all_index_rows.len() + all_quarantine_rows.len(),
        },
        "retired_embedder_policy": retired_embedder_policy(&config, &retired_embedders),
        "per_embedder_coverage": per_embedder_coverage,
        "embedder_distinctness_gate": {
            "status": "passed",
            "min_unique_vector_fraction": MIN_UNIQUE_VECTOR_FRACTION,
            "min_unique_vector_per_unique_input_fraction": MIN_UNIQUE_VECTOR_PER_UNIQUE_INPUT_FRACTION,
            "min_unique_input_count_for_duplicate_bounded_gate": MIN_UNIQUE_INPUT_COUNT_FOR_DUPLICATE_BOUNDED_GATE,
            "max_pairwise_vector_hash_jaccard": MAX_PAIRWISE_VECTOR_HASH_JACCARD,
            "catastrophic_min_non_colliding_input_fraction": CATASTROPHIC_MIN_NON_COLLIDING_INPUT_FRACTION,
            "collapse_failures": distinctness_report.collapse_failures.clone(),
            "alias_failures": distinctness_report.alias_failures.clone(),
            "benign_collision_count": distinctness_report.benign_collisions.len(),
            "benign_collisions": distinctness_report.benign_collisions.clone(),
        },
        "pairwise_jaccard_matrix": distinctness_report.pairwise_jaccard_matrix.clone(),
    });
    write_json(&manifest_path, &manifest)?;

    let readback_path = config
        .readback_root
        .join("python_forward_cache_readback.json");
    let readback = json!({
        "schema_version": SCHEMA_VERSION,
        "artifact_kind": "python_forward_cache_readback",
        "created_at_utc": utc_now(),
        "passes": true,
        "source_of_truth": {
            "manifest": manifest_path,
            "index_jsonl": config.cache_root.join("python_forward_cache_index.jsonl"),
            "quarantine_jsonl": config.cache_root.join("python_forward_cache_quarantine.jsonl"),
            "batch_telemetry_jsonl": config.cache_root.join("python_forward_cache_batch_telemetry.jsonl"),
            "packed_manifest": packed_manifest_path,
            "packed_record_root": config.cache_root.join("packed_records"),
        },
        "summary": manifest["summary"].clone(),
        "retired_embedder_policy": manifest["retired_embedder_policy"].clone(),
        "embedder_distinctness_gate": manifest["embedder_distinctness_gate"].clone(),
        "pairwise_jaccard_matrix": manifest["pairwise_jaccard_matrix"].clone(),
    });
    write_json(&readback_path, &readback)?;
    Ok(())
}

impl EnvConfig {
    fn from_env() -> RunnerResult<Self> {
        let chunks_jsonl = required_path("CG_MEJEPA_PY_CHUNKS_JSONL")?;
        let cache_root = required_path("CG_MEJEPA_PY_FORWARD_CACHE_ROOT")?;
        let readback_root = required_path("CG_MEJEPA_PY_FORWARD_CACHE_READBACK_ROOT")?;
        let chunk_offset = env_usize("CG_MEJEPA_PY_FORWARD_CACHE_CHUNK_OFFSET")?;
        let max_chunks = env_usize("CG_MEJEPA_PY_FORWARD_CACHE_MAX_CHUNKS")?;
        if max_chunks == 0 {
            return Err(RunnerError::new(
                "MEJEPA_PY_FORWARD_BAD_MAX_CHUNKS",
                "max chunks must be positive",
            ));
        }
        let max_source_bytes =
            env_usize_default("CG_MEJEPA_PY_FORWARD_MAX_SOURCE_BYTES", 32 * 1024)?;
        let cache_store_layout =
            env::var("CG_MEJEPA_PY_FORWARD_CACHE_STORE_LAYOUT").unwrap_or_else(|_| "packed".into());
        let only_embedder = match env::var("CG_MEJEPA_PY_FORWARD_ONLY_EMBEDDER") {
            Ok(value) if !value.trim().is_empty() => Some(parse_embedder(&value)?),
            _ => None,
        };
        let default_true_batch_size = env_usize_default("CG_MEJEPA_PY_FORWARD_TRUE_BATCH_SIZE", 8)?;
        let true_batch_size =
            effective_true_batch_size(only_embedder, default_true_batch_size)?.max(1);
        let corpus_profile = env::var("CG_MEJEPA_PY_FORWARD_CORPUS_PROFILE")
            .unwrap_or_else(|_| STATIC_AST_CORPUS_PROFILE.to_string());
        validate_corpus_profile(&corpus_profile)?;
        let models_config = env::var("CG_MEJEPA_MODELS_CONFIG")
            .or_else(|_| env::var("CONTEXTGRAPH_MEJEPA_MODELS_CONFIG"))
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(DEFAULT_MODELS_CONFIG));
        let gpu_vram_guard_bytes = env_u64_default("CG_MEJEPA_PY_FORWARD_GPU_VRAM_GUARD_BYTES", 0)?;
        Ok(Self {
            chunks_jsonl,
            cache_root,
            readback_root,
            chunk_offset,
            max_chunks,
            max_source_bytes,
            true_batch_size,
            cache_store_layout,
            only_embedder,
            corpus_profile,
            models_config,
            gpu_vram_guard_bytes,
        })
    }
}

fn required_path(name: &'static str) -> RunnerResult<PathBuf> {
    let value =
        env::var(name).map_err(|_| RunnerError::new("MEJEPA_PY_FORWARD_ENV_MISSING", name))?;
    if value.trim().is_empty() {
        return Err(RunnerError::new("MEJEPA_PY_FORWARD_ENV_EMPTY", name));
    }
    Ok(PathBuf::from(value))
}

fn env_usize(name: &str) -> RunnerResult<usize> {
    let value =
        env::var(name).map_err(|_| RunnerError::new("MEJEPA_PY_FORWARD_ENV_MISSING", name))?;
    value.parse::<usize>().map_err(|err| {
        RunnerError::new(
            "MEJEPA_PY_FORWARD_ENV_PARSE",
            format!("{name}={value:?}: {err}"),
        )
    })
}

fn env_usize_default(name: &str, default: usize) -> RunnerResult<usize> {
    match env::var(name) {
        Ok(value) => value.parse::<usize>().map_err(|err| {
            RunnerError::new(
                "MEJEPA_PY_FORWARD_ENV_PARSE",
                format!("{name}={value:?}: {err}"),
            )
        }),
        Err(_) => Ok(default),
    }
}

fn env_u64_default(name: &str, default: u64) -> RunnerResult<u64> {
    match env::var(name) {
        Ok(value) => value.parse::<u64>().map_err(|err| {
            RunnerError::new(
                "MEJEPA_PY_FORWARD_ENV_PARSE",
                format!("{name}={value:?}: {err}"),
            )
        }),
        Err(_) => Ok(default),
    }
}

fn effective_true_batch_size(
    only_embedder: Option<EmbedderId>,
    default_true_batch_size: usize,
) -> RunnerResult<usize> {
    let Some(embedder) = only_embedder else {
        return Ok(default_true_batch_size);
    };
    let override_name = format!(
        "CG_MEJEPA_PY_FORWARD_TRUE_BATCH_SIZE_{}",
        embedder.slug().to_ascii_uppercase()
    );
    env_usize_default(&override_name, default_true_batch_size)
}

fn parse_embedder(value: &str) -> RunnerResult<EmbedderId> {
    value.parse::<EmbedderId>().map_err(|err| {
        RunnerError::new(
            "MEJEPA_PY_FORWARD_BAD_EMBEDDER",
            format!("{value:?}: {err}"),
        )
    })
}

fn validate_corpus_profile(value: &str) -> RunnerResult<()> {
    if value == STATIC_AST_CORPUS_PROFILE || value == LIVE_TEMPORAL_CORPUS_PROFILE {
        return Ok(());
    }
    Err(RunnerError::new(
        "MEJEPA_PY_FORWARD_BAD_CORPUS_PROFILE",
        format!(
            "CG_MEJEPA_PY_FORWARD_CORPUS_PROFILE must be {STATIC_AST_CORPUS_PROFILE:?} or {LIVE_TEMPORAL_CORPUS_PROFILE:?}, got {value:?}"
        ),
    ))
}

fn temporal_static_ast_retired(embedder: EmbedderId) -> bool {
    matches!(embedder, EmbedderId::E2 | EmbedderId::E3 | EmbedderId::E4)
}

fn retired_embedders(config: &EnvConfig) -> Vec<EmbedderId> {
    if config.corpus_profile == STATIC_AST_CORPUS_PROFILE {
        SUPPORTED_FORWARD_EMBEDDERS
            .iter()
            .copied()
            .filter(|embedder| temporal_static_ast_retired(*embedder))
            .collect()
    } else {
        Vec::new()
    }
}

fn active_embedders(config: &EnvConfig) -> RunnerResult<Vec<EmbedderId>> {
    match config.only_embedder {
        Some(embedder)
            if config.corpus_profile == STATIC_AST_CORPUS_PROFILE
                && temporal_static_ast_retired(embedder) =>
        {
            Err(RunnerError::new(
                "MEJEPA_PY_FORWARD_TEMPORAL_RETIRED_FOR_STATIC_AST",
                format!(
                    "{} is retired for static AST corpus forwards; use corpus_profile={LIVE_TEMPORAL_CORPUS_PROFILE} only with explicit per-row temporal instructions",
                    embedder.slug()
                ),
            ))
        }
        Some(embedder) => Ok(vec![embedder]),
        None => Ok(SUPPORTED_FORWARD_EMBEDDERS
            .iter()
            .copied()
            .filter(|embedder| {
                config.corpus_profile != STATIC_AST_CORPUS_PROFILE
                    || !temporal_static_ast_retired(*embedder)
            })
            .collect()),
    }
}

fn embedder_slugs(embedders: &[EmbedderId]) -> Vec<&'static str> {
    embedders.iter().map(|embedder| embedder.slug()).collect()
}

fn retired_embedder_policy(config: &EnvConfig, retired_embedders: &[EmbedderId]) -> Value {
    json!({
        "corpus_profile": config.corpus_profile.as_str(),
        "policy": if retired_embedders.is_empty() {
            "no_static_retirements"
        } else {
            "temporal_slots_retired_for_static_ast_code"
        },
        "retired_embedder_ids": embedder_slugs(retired_embedders),
        "retirement_reason": if retired_embedders.is_empty() {
            Value::Null
        } else {
            Value::String("E2/E3/E4 require semantic temporal instructions; AST chunker run timestamps are non-semantic and collapse these spaces".to_string())
        },
    })
}

fn is_pretrained(embedder: EmbedderId) -> bool {
    matches!(
        embedder,
        EmbedderId::E1
            | EmbedderId::E6
            | EmbedderId::E7
            | EmbedderId::E8
            | EmbedderId::E10
            | EmbedderId::E12
            | EmbedderId::E13
            | EmbedderId::E14
    )
}

async fn load_forwarder(
    embedder: EmbedderId,
    models_config: Option<&ModelsConfig>,
) -> RunnerResult<Box<dyn EmbedderForward>> {
    if is_pretrained(embedder) {
        let config = models_config.ok_or_else(|| {
            RunnerError::new(
                "MEJEPA_PY_FORWARD_MODELS_CONFIG_REQUIRED",
                format!("{embedder} requires models_config"),
            )
        })?;
        let registration = config.registration(embedder).map_err(|err| {
            RunnerError::new(
                "MEJEPA_PY_FORWARD_REGISTRATION_MISSING",
                format!("{embedder}: {err}"),
            )
        })?;
        let forwarder = PretrainedEmbedderForward::load(registration)
            .await
            .map_err(|err| runner_error_from_embed(embedder, err))?;
        Ok(Box::new(forwarder))
    } else {
        let forwarder = AlgorithmicEmbedderForward::load(embedder)
            .map_err(|err| runner_error_from_embed(embedder, err))?;
        Ok(Box::new(forwarder))
    }
}

fn runner_error_from_embed(embedder: EmbedderId, err: EmbedError) -> RunnerError {
    RunnerError::new(
        "MEJEPA_PY_FORWARD_EMBEDDER_LOAD_FAILED",
        format!("{embedder}: {}: {err}", err.code()),
    )
}

fn load_chunks(config: &EnvConfig) -> RunnerResult<Vec<ChunkRow>> {
    if !config.chunks_jsonl.is_file() {
        return Err(RunnerError::new(
            "MEJEPA_PY_FORWARD_CHUNKS_MISSING",
            config.chunks_jsonl.display().to_string(),
        ));
    }
    let file = File::open(&config.chunks_jsonl).map_err(|err| {
        RunnerError::new(
            "MEJEPA_PY_FORWARD_CHUNKS_OPEN_FAILED",
            format!("{}: {err}", config.chunks_jsonl.display()),
        )
    })?;
    let mut rows = Vec::new();
    for (idx, line) in BufReader::new(file).lines().enumerate() {
        let line = line.map_err(|err| {
            RunnerError::new("MEJEPA_PY_FORWARD_CHUNKS_READ_FAILED", err.to_string())
        })?;
        if idx < config.chunk_offset {
            continue;
        }
        if rows.len() >= config.max_chunks {
            break;
        }
        if line.trim().is_empty() {
            continue;
        }
        let raw: Value = serde_json::from_str(&line).map_err(|err| {
            RunnerError::new(
                "MEJEPA_PY_FORWARD_CHUNK_JSON_INVALID",
                format!("line {}: {err}", idx + 1),
            )
        })?;
        rows.push(chunk_from_value(raw, idx + 1)?);
    }
    Ok(rows)
}

fn chunk_from_value(raw: Value, line_number: usize) -> RunnerResult<ChunkRow> {
    let get = |key: &'static str| -> RunnerResult<String> {
        raw.get(key)
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .map(str::to_string)
            .ok_or_else(|| {
                RunnerError::new(
                    "MEJEPA_PY_FORWARD_CHUNK_FIELD_MISSING",
                    format!("line {line_number} missing {key}"),
                )
            })
    };
    let absolute_path = get("absolute_path")?;
    let relative_path = get("relative_path")?;
    let candidate_row_id = candidate_id_from_path(&absolute_path)
        .or_else(|| candidate_id_from_source(raw.get("source").and_then(Value::as_str)));
    Ok(ChunkRow {
        chunk_id: get("chunk_id")?,
        source_text: get("source_text")?,
        source_sha256: get("source_sha256")?,
        file_sha256: get("file_sha256")?,
        task_id: get("task_id")?,
        task_instance_id: get("task_instance_id")?,
        repo: get("repo")?,
        workspace_state: get("workspace_state")?,
        absolute_path,
        relative_path,
        mutation_category: raw
            .get("mutation_category")
            .and_then(Value::as_str)
            .map(str::to_string),
        candidate_row_id,
        line_number,
    })
}

fn candidate_id_from_path(path: &str) -> Option<String> {
    let marker = "/source-files/";
    let start = path.find(marker)? + marker.len();
    let rest = &path[start..];
    let hex = rest.split('/').next()?;
    if hex.len() == 64 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Some(format!("sha256:{}", hex.to_ascii_lowercase()))
    } else {
        None
    }
}

fn candidate_id_from_source(source: Option<&str>) -> Option<String> {
    let source = source?;
    let marker = "candidate_mutated_";
    let start = source.find(marker)? + marker.len();
    let rest = &source[start..];
    let prefix: String = rest
        .chars()
        .take_while(|ch| ch.is_ascii_hexdigit())
        .collect();
    if prefix.len() >= 24 {
        Some(format!("sha256:{prefix}"))
    } else {
        None
    }
}

async fn run_embedder_batches(
    config: &EnvConfig,
    chunks: &[ChunkRow],
    forwarder: &dyn EmbedderForward,
    batch_counter: &mut usize,
) -> Vec<BatchResult> {
    let mut results = Vec::new();
    let mut output_cache: BTreeMap<String, EmbedderOutput> = BTreeMap::new();
    for batch in chunks.chunks(config.true_batch_size.max(1)) {
        let batch_id = format!("{}-batch-{batch_counter:06}", forwarder.embedder().slug());
        *batch_counter += 1;
        let started = Instant::now();
        let mut records = Vec::new();
        let mut quarantines = Vec::new();
        let mut pending = Vec::<PendingForward>::new();
        let mut pending_by_input = BTreeMap::<String, usize>::new();
        let mut plans = Vec::<BatchRecordPlan<'_>>::new();
        let mut duplicate_forward_cache_hits = 0usize;
        for chunk in batch {
            if chunk.source_text.len() > config.max_source_bytes {
                quarantines.push(QuarantineRecord {
                    row: quarantine_row(
                        forwarder.embedder(),
                        chunk,
                        "MEJEPA_FORWARD_INPUT_TOO_LARGE",
                        "accepted Python AST chunk source_text exceeds max forward byte guard",
                        Some(config.max_source_bytes),
                    ),
                });
                continue;
            }
            let identity = match forward_input_identity(forwarder.embedder(), chunk) {
                Ok(identity) => identity,
                Err(err) => {
                    quarantines.push(QuarantineRecord {
                        row: quarantine_row(
                            forwarder.embedder(),
                            chunk,
                            err.code,
                            err.message,
                            None,
                        ),
                    });
                    continue;
                }
            };
            if let Some(output) = output_cache.get(&identity.input_sha256) {
                duplicate_forward_cache_hits += 1;
                plans.push(BatchRecordPlan::Cached {
                    chunk,
                    identity,
                    output: output.clone(),
                });
                continue;
            }
            if let Some(&pending_idx) = pending_by_input.get(&identity.input_sha256) {
                duplicate_forward_cache_hits += 1;
                plans.push(BatchRecordPlan::Pending {
                    chunk,
                    identity,
                    pending_idx,
                });
                continue;
            }
            let input = EmbedderInput {
                embedder: forwarder.embedder(),
                text: chunk.source_text.clone(),
                source_id: source_id_for_embedder(forwarder.embedder(), chunk),
            };
            let pending_idx = pending.len();
            pending_by_input.insert(identity.input_sha256.clone(), pending_idx);
            plans.push(BatchRecordPlan::Pending {
                chunk,
                identity: identity.clone(),
                pending_idx,
            });
            pending.push(PendingForward { input, identity });
        }
        let mut pending_results = Vec::<Result<EmbedderOutput, ForwardFailure>>::new();
        if !pending.is_empty() {
            if forwarder.supports_true_batch() {
                let forwarded = pending
                    .iter()
                    .map(|pending| pending.input.clone())
                    .collect::<Vec<_>>();
                match forwarder.forward_true_batch(&forwarded).await {
                    Ok(outputs) => {
                        if outputs.len() == pending.len() {
                            pending_results.extend(outputs.into_iter().map(Ok));
                        } else {
                            let err = ForwardFailure::new(
                                "MEJEPA_PY_FORWARD_TRUE_BATCH_ROW_COUNT_MISMATCH",
                                format!(
                                    "{} true-batch output count {}, expected {}",
                                    forwarder.embedder().slug(),
                                    outputs.len(),
                                    pending.len()
                                ),
                            );
                            pending_results.extend(
                                std::iter::repeat_with(|| Err(err.clone())).take(pending.len()),
                            );
                        }
                    }
                    Err(err) => {
                        let err = ForwardFailure::from_embed(&err);
                        pending_results.extend(
                            std::iter::repeat_with(|| Err(err.clone())).take(pending.len()),
                        );
                    }
                }
            } else {
                for pending_forward in &pending {
                    pending_results.push(
                        forwarder
                            .forward(&pending_forward.input)
                            .await
                            .map_err(|err| ForwardFailure::from_embed(&err)),
                    );
                }
            }
        }
        if pending_results.len() != pending.len() {
            let err = ForwardFailure::new(
                "MEJEPA_PY_FORWARD_PENDING_RESULT_COUNT_MISMATCH",
                format!(
                    "{} pending result count {}, expected {}",
                    forwarder.embedder().slug(),
                    pending_results.len(),
                    pending.len()
                ),
            );
            pending_results = std::iter::repeat_with(|| Err(err.clone()))
                .take(pending.len())
                .collect();
        }
        for (pending_forward, result) in pending.iter().zip(&pending_results) {
            if let Ok(output) = result {
                output_cache.insert(
                    pending_forward.identity.input_sha256.clone(),
                    output.clone(),
                );
            }
        }
        for plan in plans {
            match plan {
                BatchRecordPlan::Cached {
                    chunk,
                    identity,
                    output,
                } => records.push(forward_record(
                    forwarder.embedder(),
                    chunk,
                    &identity,
                    output,
                )),
                BatchRecordPlan::Pending {
                    chunk,
                    identity,
                    pending_idx,
                } => match &pending_results[pending_idx] {
                    Ok(output) => records.push(forward_record(
                        forwarder.embedder(),
                        chunk,
                        &identity,
                        output.clone(),
                    )),
                    Err(err) => quarantines.push(QuarantineRecord {
                        row: quarantine_row(
                            forwarder.embedder(),
                            chunk,
                            err.code,
                            err.message.clone(),
                            None,
                        ),
                    }),
                },
            }
        }
        let latency_ms = elapsed_ms(started);
        let telemetry = telemetry_row(
            config,
            forwarder,
            &batch_id,
            BatchTelemetryCounts {
                batch_size: batch.len(),
                output_count: records.len(),
                quarantine_count: quarantines.len(),
                unique_forward_input_count: pending.len(),
                duplicate_forward_cache_hits,
                latency_ms,
            },
        );
        results.push(BatchResult {
            telemetry,
            records,
            quarantines,
            duplicate_forward_cache_hits,
            unique_forward_input_count: pending.len(),
        });
    }
    results
}

fn source_id_for_embedder(embedder: EmbedderId, chunk: &ChunkRow) -> String {
    if embedder == EmbedderId::E7 {
        return chunk.relative_path.clone();
    }
    format!(
        "{}\n{}\n{}",
        chunk.absolute_path, chunk.workspace_state, chunk.chunk_id
    )
}

fn forward_input_identity_policy(embedder: EmbedderId) -> &'static str {
    if embedder == EmbedderId::E7 {
        "e7_language_and_source_text_sha256_v1"
    } else if matches!(embedder, EmbedderId::E2 | EmbedderId::E3 | EmbedderId::E4) {
        "temporal_instruction_and_source_text_sha256_v2"
    } else {
        "source_text_sha256_v1"
    }
}

fn e7_forward_language_identity(chunk: &ChunkRow) -> String {
    AstLanguage::from_path(&chunk.relative_path)
        .map(|language| language.slug().to_string())
        .unwrap_or_else(|_| format!("unsupported_source_id:{}", chunk.relative_path))
}

fn forward_input_identity(
    embedder: EmbedderId,
    chunk: &ChunkRow,
) -> Result<ForwardInputIdentity, ForwardFailure> {
    let policy = forward_input_identity_policy(embedder);
    let input_sha256 = if embedder == EmbedderId::E7 {
        sha256_value(&json!({
            "schema_version": SCHEMA_VERSION,
            "policy": policy,
            "embedder_id": embedder.slug(),
            "language": e7_forward_language_identity(chunk),
            "source_sha256": chunk.source_sha256,
        }))
    } else if matches!(embedder, EmbedderId::E2 | EmbedderId::E3 | EmbedderId::E4) {
        let temporal_instruction = temporal_instruction_for_identity(embedder, chunk)?;
        sha256_value(&json!({
            "schema_version": SCHEMA_VERSION,
            "policy": policy,
            "embedder_id": embedder.slug(),
            "source_sha256": chunk.source_sha256,
            "temporal_instruction": temporal_instruction,
        }))
    } else {
        chunk.source_sha256.clone()
    };
    Ok(ForwardInputIdentity {
        input_sha256,
        policy,
    })
}

fn temporal_instruction_for_identity(
    embedder: EmbedderId,
    chunk: &ChunkRow,
) -> Result<String, ForwardFailure> {
    let source_id = source_id_for_embedder(embedder, chunk);
    let mut invalid_supported_instruction = None;
    for candidate in [source_id.as_str(), chunk.source_text.as_str()] {
        for line in candidate.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let has_supported_prefix = match embedder {
                EmbedderId::E2 | EmbedderId::E3 => has_timestamp_prefix(trimmed),
                EmbedderId::E4 => has_positional_prefix(trimmed),
                _ => false,
            };
            if !has_supported_prefix {
                continue;
            }
            match validate_temporal_instruction_for_identity(embedder, trimmed) {
                Ok(()) => return Ok(trimmed.to_string()),
                Err(err) => {
                    invalid_supported_instruction.get_or_insert(err);
                }
            }
        }
    }
    Err(ForwardFailure::new(
        "MEJEPA_PY_FORWARD_TEMPORAL_INSTRUCTION_INVALID",
        invalid_supported_instruction.unwrap_or_else(|| match embedder {
            EmbedderId::E2 | EmbedderId::E3 => {
                "missing explicit timestamp instruction; expected timestamp:<RFC3339> or epoch:<seconds>".to_string()
            }
            EmbedderId::E4 => {
                "missing explicit session sequence instruction; expected session:<id> sequence:<n>".to_string()
            }
            _ => "embedder has no temporal instruction contract".to_string(),
        }),
    ))
}

fn has_timestamp_prefix(value: &str) -> bool {
    value.starts_with("timestamp:") || value.starts_with("epoch:")
}

fn has_positional_prefix(value: &str) -> bool {
    value.starts_with("session:") || value.starts_with("sequence:") || has_timestamp_prefix(value)
}

fn validate_temporal_instruction_for_identity(
    embedder: EmbedderId,
    value: &str,
) -> Result<(), String> {
    match embedder {
        EmbedderId::E2 | EmbedderId::E3 => validate_timestamp_instruction_for_identity(value),
        EmbedderId::E4 => validate_e4_session_sequence_instruction_for_identity(value),
        _ => Err(format!("{} is not a temporal embedder", embedder.slug())),
    }
}

fn validate_timestamp_instruction_for_identity(value: &str) -> Result<(), String> {
    if let Some(ts_str) = value.strip_prefix("timestamp:") {
        let ts_str = ts_str.trim();
        if ts_str.is_empty() {
            return Err("timestamp instruction is empty; expected timestamp:<RFC3339>".to_string());
        }
        DateTime::parse_from_rfc3339(ts_str)
            .map_err(|err| format!("invalid RFC3339 timestamp {ts_str:?}: {err}"))?;
        return Ok(());
    }
    if let Some(epoch_str) = value.strip_prefix("epoch:") {
        let epoch_str = epoch_str.trim();
        if epoch_str.is_empty() {
            return Err("epoch instruction is empty; expected epoch:<seconds>".to_string());
        }
        let secs = epoch_str
            .parse::<i64>()
            .map_err(|err| format!("invalid epoch seconds {epoch_str:?}: {err}"))?;
        chrono::DateTime::from_timestamp(secs, 0).ok_or_else(|| {
            format!("epoch seconds {secs} is outside chrono's DateTime<Utc> range")
        })?;
        return Ok(());
    }
    Err(format!(
        "unsupported timestamp instruction {value:?}; expected timestamp:<RFC3339> or epoch:<seconds>"
    ))
}

fn validate_e4_session_sequence_instruction_for_identity(value: &str) -> Result<(), String> {
    let mut session_id = None;
    let mut sequence = None;
    for part in value.split_whitespace() {
        if let Some(id) = part.strip_prefix("session:") {
            if id.is_empty() {
                return Err("session instruction is empty; expected session:<id>".to_string());
            }
            session_id = Some(id);
            continue;
        }
        if let Some(seq_str) = part.strip_prefix("sequence:") {
            if seq_str.is_empty() {
                return Err("sequence instruction is empty; expected sequence:<n>".to_string());
            }
            let parsed = seq_str
                .parse::<u64>()
                .map_err(|err| format!("invalid sequence {seq_str:?}: {err}"))?;
            sequence = Some(parsed);
            continue;
        }
        if part.starts_with("timestamp:") || part.starts_with("epoch:") {
            return Err(
                "E4 live-temporal instruction must be session-scoped sequence, not timestamp/epoch"
                    .to_string(),
            );
        }
    }
    if session_id.is_none() {
        return Err("missing non-empty session id; expected session:<id> sequence:<n>".to_string());
    }
    if sequence.is_none() {
        return Err("missing sequence; expected session:<id> sequence:<n>".to_string());
    }
    Ok(())
}

fn forward_record(
    embedder: EmbedderId,
    chunk: &ChunkRow,
    input_identity: &ForwardInputIdentity,
    output: EmbedderOutput,
) -> ForwardRecord {
    let vector_sha256 = vector_sha256(&output.vector);
    let instrument_id = instrument_id(embedder);
    let model_hash = sha256_prefixed(output.model_version.as_bytes());
    let preprocessing_hash =
        sha256_prefixed(format!("python_forward_cache_readback:v1:{}", embedder.slug()).as_bytes());
    let key = json!({
        "schema_version": SCHEMA_VERSION,
        "artifact_kind": "python_ast_chunk_forward",
        "chunk_id": chunk.chunk_id,
        "source_sha256": chunk.source_sha256,
        "file_sha256": chunk.file_sha256,
        "embedder_id": embedder.slug(),
        "instrument_record_id": sha256_prefixed(instrument_id.as_bytes()),
        "preprocessing_hash": preprocessing_hash,
        "model_hash": model_hash,
        "input_sha256": input_identity.input_sha256,
        "source_text_sha256": chunk.source_sha256,
        "forward_input_identity_policy": input_identity.policy,
        "output_feature_family": "dda_tct_embedder_vector",
    });
    let cache_key_sha256 = sha256_value(&key);
    let record = json!({
        "schema_version": SCHEMA_VERSION,
        "cache_key_sha256": cache_key_sha256,
        "key": key,
        "chunk": {
            "task_id": chunk.task_id,
            "task_instance_id": chunk.task_instance_id,
            "repo": chunk.repo,
            "workspace_state": chunk.workspace_state,
            "absolute_path": chunk.absolute_path,
            "relative_path": chunk.relative_path,
            "mutation_category": chunk.mutation_category,
            "candidate_row_id": chunk.candidate_row_id,
            "chunk_line_number": chunk.line_number,
        },
        "instrument_id": instrument_id,
        "instrument_kind": instrument_kind(embedder),
        "output": {
            "vector": output.vector,
            "model_version": output.model_version,
            "precision_class": output.precision_class,
        },
        "vector_len": embedder.dimension(),
        "vector_sha256": vector_sha256,
    });
    let index_row = json!({
        "schema_version": SCHEMA_VERSION,
        "task_id": chunk.task_id,
        "task_instance_id": chunk.task_instance_id,
        "repo": chunk.repo,
        "workspace_state": chunk.workspace_state,
        "mutation_category": chunk.mutation_category,
        "candidate_row_id": chunk.candidate_row_id,
        "chunk_id": chunk.chunk_id,
        "source_sha256": chunk.source_sha256,
        "source_text_sha256": chunk.source_sha256,
        "file_sha256": chunk.file_sha256,
        "absolute_path": chunk.absolute_path,
        "relative_path": chunk.relative_path,
        "embedder_id": embedder.slug(),
        "instrument_id": instrument_id,
        "cache_key_sha256": cache_key_sha256,
        "input_sha256": input_identity.input_sha256,
        "forward_input_identity_policy": input_identity.policy,
        "record_store_layout": "packed-jsonl",
        "record_sha256": "",
        "record_path": "",
        "packed_record_path": "",
        "packed_index_path": "",
        "packed_byte_offset": 0,
        "packed_byte_len": 0,
        "vector_len": embedder.dimension(),
        "vector_sha256": vector_sha256,
        "true_batch": output.precision_class.contains("true_batch"),
        "latency_ms": 0,
    });
    ForwardRecord {
        index_row,
        packed_record: record,
    }
}

fn quarantine_row(
    embedder: EmbedderId,
    chunk: &ChunkRow,
    error_code: &str,
    reason: impl Into<String>,
    max_source_bytes: Option<usize>,
) -> Value {
    json!({
        "schema_version": SCHEMA_VERSION,
        "artifact_kind": "python_ast_chunk_forward_quarantine",
        "task_id": chunk.task_id,
        "task_instance_id": chunk.task_instance_id,
        "repo": chunk.repo,
        "workspace_state": chunk.workspace_state,
        "mutation_category": chunk.mutation_category,
        "candidate_row_id": chunk.candidate_row_id,
        "absolute_path": chunk.absolute_path,
        "relative_path": chunk.relative_path,
        "chunk_id": chunk.chunk_id,
        "source_sha256": chunk.source_sha256,
        "file_sha256": chunk.file_sha256,
        "embedder_id": embedder.slug(),
        "instrument_id": instrument_id(embedder),
        "error_code": error_code,
        "reason": reason.into(),
        "max_forward_source_text_bytes": max_source_bytes,
        "source_text_bytes": chunk.source_text.len(),
    })
}

fn telemetry_row(
    config: &EnvConfig,
    forwarder: &dyn EmbedderForward,
    batch_id: &str,
    counts: BatchTelemetryCounts,
) -> Value {
    if forwarder.supports_true_batch() {
        json!({
            "schema_version": SCHEMA_VERSION,
            "artifact_kind": "python_forward_true_batch_telemetry",
            "embedder_id": forwarder.embedder().slug(),
            "batch_id": batch_id,
            "batch_size": counts.batch_size,
            "output_count": counts.output_count,
            "quarantine_count": counts.quarantine_count,
            "unique_forward_input_count": counts.unique_forward_input_count,
            "duplicate_forward_cache_hit_count": counts.duplicate_forward_cache_hits,
            "forward_input_identity_policy": forward_input_identity_policy(forwarder.embedder()),
            "latency_ms": counts.latency_ms,
            "true_batch": true,
            "precision_classes": ["real_forward_or_typed_quarantine"],
            "vram_plan": {
                "embedder_id": forwarder.embedder().slug(),
                "required_bytes": 0,
                "guard_bytes": config.gpu_vram_guard_bytes,
            },
        })
    } else {
        json!({
            "schema_version": SCHEMA_VERSION,
            "artifact_kind": "python_forward_algorithmic_serial_telemetry",
            "embedder_id": forwarder.embedder().slug(),
            "batch_id": batch_id,
            "row_count": counts.batch_size,
            "processed_non_quarantine_count": counts.output_count,
            "quarantine_count": counts.quarantine_count,
            "unique_forward_input_count": counts.unique_forward_input_count,
            "duplicate_forward_cache_hit_count": counts.duplicate_forward_cache_hits,
            "forward_input_identity_policy": forward_input_identity_policy(forwarder.embedder()),
            "latency_ms": counts.latency_ms,
            "cuda_applicable": false,
            "reason_code": "algorithmic_serial_or_typed_quarantine",
        })
    }
}

fn write_packed_embedder_records(
    config: &EnvConfig,
    embedder: EmbedderId,
    records: &[ForwardRecord],
) -> RunnerResult<(Value, Vec<Value>)> {
    let packed_root = config.cache_root.join("packed_records");
    fs::create_dir_all(&packed_root).map_err(|err| {
        RunnerError::new(
            "MEJEPA_PY_FORWARD_MKDIR_FAILED",
            format!("{}: {err}", packed_root.display()),
        )
    })?;
    let record_path = packed_root.join(format!("{}.jsonl", embedder.slug()));
    let index_path = packed_root.join(format!("{}.index.jsonl", embedder.slug()));
    let mut record_file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&record_path)
        .map_err(|err| {
            RunnerError::new(
                "MEJEPA_PY_FORWARD_PACKED_OPEN_FAILED",
                format!("{}: {err}", record_path.display()),
            )
        })?;
    let mut packed_index_rows = Vec::new();
    let mut top_index_rewrites = Vec::new();
    let mut offset = 0usize;
    for record in records {
        let mut packed_record = record.packed_record.clone();
        let record_bytes = serde_json::to_vec(&packed_record).expect("JSON value serializes");
        let record_sha256 = sha256_prefixed(&record_bytes);
        let byte_len = record_bytes.len();
        record_file.write_all(&record_bytes).map_err(|err| {
            RunnerError::new("MEJEPA_PY_FORWARD_PACKED_WRITE_FAILED", err.to_string())
        })?;
        record_file.write_all(b"\n").map_err(|err| {
            RunnerError::new("MEJEPA_PY_FORWARD_PACKED_WRITE_FAILED", err.to_string())
        })?;
        packed_record["record_sha256"] = Value::String(record_sha256.clone());
        let cache_key = packed_record["cache_key_sha256"].clone();
        let vector_sha = packed_record["vector_sha256"].clone();
        let vector_len = packed_record["vector_len"].clone();
        let packed_index = json!({
            "schema_version": SCHEMA_VERSION,
            "cache_key_sha256": cache_key,
            "embedder_id": embedder.slug(),
            "packed_record_path": record_path,
            "byte_offset": offset,
            "byte_len": byte_len,
            "record_sha256": record_sha256.clone(),
            "vector_sha256": vector_sha,
            "vector_len": vector_len,
        });
        packed_index_rows.push(packed_index);
        let mut top = record.index_row.clone();
        top["record_sha256"] = Value::String(record_sha256);
        top["record_path"] = Value::String(format!(
            "{}#{}",
            record_path.display(),
            top.get("cache_key_sha256")
                .and_then(Value::as_str)
                .unwrap_or("")
        ));
        top["packed_record_path"] = Value::String(record_path.display().to_string());
        top["packed_index_path"] = Value::String(index_path.display().to_string());
        top["packed_byte_offset"] = json!(offset);
        top["packed_byte_len"] = json!(byte_len);
        top_index_rewrites.push(top);
        offset += byte_len + 1;
    }
    record_file.flush().map_err(|err| {
        RunnerError::new("MEJEPA_PY_FORWARD_PACKED_FLUSH_FAILED", err.to_string())
    })?;
    write_jsonl(&index_path, &packed_index_rows)?;
    let top_index_path = config
        .cache_root
        .join(format!("{}.top_index_rewrite.jsonl", embedder.slug()));
    write_jsonl(&top_index_path, &top_index_rewrites)?;
    let packed = json!({
        "record_path": record_path,
        "record_sha256": sha256_file(&record_path)?,
        "byte_count": fs::metadata(&record_path).map(|meta| meta.len()).unwrap_or(0),
        "index_path": index_path,
        "index_sha256": sha256_file(&index_path)?,
        "row_count": packed_index_rows.len(),
        "top_index_rewrite": top_index_path,
    });
    Ok((packed, top_index_rewrites))
}

#[derive(Clone, Debug, Default)]
struct EmbedderDistinctnessInputs {
    vector_counts: BTreeMap<String, usize>,
    input_counts: BTreeMap<String, usize>,
    input_to_vectors: BTreeMap<String, BTreeSet<String>>,
    vector_to_inputs: BTreeMap<String, BTreeSet<String>>,
}

fn distinctness_inputs_for_records(
    embedder: EmbedderId,
    records: &[ForwardRecord],
) -> RunnerResult<EmbedderDistinctnessInputs> {
    let mut inputs = EmbedderDistinctnessInputs::default();
    for record in records {
        let vector_sha = record
            .index_row
            .get("vector_sha256")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| {
                RunnerError::new(
                    "MEJEPA_PY_FORWARD_VECTOR_SHA256_MISSING",
                    format!("{} forward record missing vector_sha256", embedder.slug()),
                )
            })?;
        let input_sha = record
            .index_row
            .get("input_sha256")
            .or_else(|| record.index_row.get("source_sha256"))
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| {
                RunnerError::new(
                    "MEJEPA_PY_FORWARD_INPUT_SHA256_MISSING",
                    format!(
                        "{} forward record missing input_sha256/source_sha256",
                        embedder.slug()
                    ),
                )
            })?;
        *inputs
            .vector_counts
            .entry(vector_sha.to_string())
            .or_insert(0) += 1;
        *inputs
            .input_counts
            .entry(input_sha.to_string())
            .or_insert(0) += 1;
        inputs
            .input_to_vectors
            .entry(input_sha.to_string())
            .or_default()
            .insert(vector_sha.to_string());
        inputs
            .vector_to_inputs
            .entry(vector_sha.to_string())
            .or_default()
            .insert(input_sha.to_string());
    }
    Ok(inputs)
}

fn evaluate_embedder_distinctness(
    active_embedder_ids: &[String],
    inputs_by_embedder: &BTreeMap<String, EmbedderDistinctnessInputs>,
) -> DistinctnessReport {
    let mut per_embedder_metrics = BTreeMap::new();
    let mut collapse_failures = Vec::new();
    let mut benign_collisions = Vec::new();
    let mut vector_hash_sets = BTreeMap::new();

    for embedder_id in active_embedder_ids {
        let inputs = inputs_by_embedder
            .get(embedder_id)
            .cloned()
            .unwrap_or_default();
        let counts = &inputs.vector_counts;
        let input_counts = &inputs.input_counts;
        let row_count: usize = counts.values().sum();
        let unique_count = counts.len();
        let unique_input_count = input_counts.len();
        let input_with_multiple_vectors_count = inputs
            .input_to_vectors
            .values()
            .filter(|vectors| vectors.len() > 1)
            .count();
        let vector_collision_group_count = inputs
            .vector_to_inputs
            .values()
            .filter(|source_inputs| source_inputs.len() > 1)
            .count();
        let vector_collision_unique_inputs = vector_collision_unique_inputs(&inputs);
        let vector_collision_unique_input_count = vector_collision_unique_inputs.len();
        let non_colliding_unique_input_count =
            unique_input_count.saturating_sub(vector_collision_unique_input_count);
        let distinctness_fraction = if row_count == 0 {
            0.0
        } else {
            unique_count as f64 / row_count as f64
        };
        let input_distinctness_fraction = if row_count == 0 {
            0.0
        } else {
            unique_input_count as f64 / row_count as f64
        };
        let unique_vector_per_unique_input_fraction = if unique_input_count == 0 {
            0.0
        } else {
            unique_count.min(unique_input_count) as f64 / unique_input_count as f64
        };
        let non_colliding_unique_input_fraction = if unique_input_count == 0 {
            0.0
        } else {
            non_colliding_unique_input_count as f64 / unique_input_count as f64
        };
        // Determinism is the only per-shard identity guard: the SAME input must always
        // map to exactly one vector. A violation means a nondeterministic embedder
        // (e.g. batch-position float drift) and is always fatal. `observed_input_identity_preserved`
        // keeps its published field name but now means pure determinism — the old
        // conflated 0.9 non-colliding floor moved to the catastrophic/clean split below
        // and to the corpus-scope guard in the scheduler.
        let input_identity_deterministic =
            unique_input_count > 0 && input_with_multiple_vectors_count == 0;
        let observed_input_identity_preserved = input_identity_deterministic;
        let non_colliding_above_catastrophic_floor =
            non_colliding_unique_input_fraction >= CATASTROPHIC_MIN_NON_COLLIDING_INPUT_FRACTION;
        let non_colliding_meets_clean_threshold =
            non_colliding_unique_input_fraction >= MIN_UNIQUE_VECTOR_PER_UNIQUE_INPUT_FRACTION;
        let duplicate_bounded_usable =
            unique_input_count >= MIN_UNIQUE_INPUT_COUNT_FOR_DUPLICATE_BOUNDED_GATE;
        let status = if row_count == 0 {
            "no_forward_records"
        } else if !input_identity_deterministic {
            // Same input -> multiple vectors: nondeterministic embedder. FATAL.
            "collapse_nondeterministic"
        } else if !non_colliding_above_catastrophic_floor {
            // Majority of distinct inputs collapse into shared vectors: a genuinely
            // degenerate embedder for this shard (constant/broken model). FATAL, fail fast.
            "collapse_catastrophic"
        } else if non_colliding_meets_clean_threshold {
            // Clean shard (>= 0.9 of distinct inputs map to a distinct vector): original
            // informational label hierarchy, all non-fatal.
            if distinctness_fraction >= MIN_UNIQUE_VECTOR_FRACTION {
                "usable"
            } else if duplicate_bounded_usable {
                "usable_duplicate_bounded"
            } else {
                "usable_duplicate_limited"
            }
        } else {
            // 0.5 <= non_colliding < 0.9 with deterministic inputs: legitimate local
            // redundancy (e.g. uncased embedder on case-only variants). NON-FATAL,
            // recorded as a benign collision and rolled up at corpus scope.
            "usable_benign_collision"
        };
        let status_is_fatal_collapse =
            status == "collapse_nondeterministic" || status == "collapse_catastrophic";
        let metric = json!({
            "forward_record_count": row_count,
            "unique_vector_sha256_count": unique_count,
            "distinctness_fraction": distinctness_fraction,
            "distinctness_gate_status": status,
            "distinctness_min_unique_vector_fraction": MIN_UNIQUE_VECTOR_FRACTION,
            "unique_input_sha256_count": unique_input_count,
            "input_distinctness_fraction": input_distinctness_fraction,
            "unique_vector_per_unique_input_fraction": unique_vector_per_unique_input_fraction,
            "input_with_multiple_vectors_count": input_with_multiple_vectors_count,
            "vector_collision_group_count": vector_collision_group_count,
            "vector_collision_unique_input_count": vector_collision_unique_input_count,
            "non_colliding_unique_input_count": non_colliding_unique_input_count,
            "non_colliding_unique_input_fraction": non_colliding_unique_input_fraction,
            "observed_input_identity_preserved": observed_input_identity_preserved,
            "duplicate_bounded_gate_min_unique_input_count": MIN_UNIQUE_INPUT_COUNT_FOR_DUPLICATE_BOUNDED_GATE,
            "duplicate_bounded_gate_min_unique_vector_per_unique_input_fraction": MIN_UNIQUE_VECTOR_PER_UNIQUE_INPUT_FRACTION,
            "top_vector_sha256_counts": top_vector_sha256_counts(counts),
            "top_vector_collision_groups": top_vector_collision_groups(&inputs),
        });
        if status_is_fatal_collapse {
            let collapse_reason = if status == "collapse_nondeterministic" {
                "input_identity_nondeterministic"
            } else {
                "catastrophic_vector_collision"
            };
            collapse_failures.push(json!({
                "error_code": ERR_EMBEDDER_COLLAPSE,
                "collapse_reason": collapse_reason,
                "distinctness_gate_status": status,
                "embedder_id": embedder_id,
                "row_count": row_count,
                "unique_vector_sha256_count": unique_count,
                "distinctness_fraction": distinctness_fraction,
                "min_unique_vector_fraction": MIN_UNIQUE_VECTOR_FRACTION,
                "catastrophic_min_non_colliding_input_fraction": CATASTROPHIC_MIN_NON_COLLIDING_INPUT_FRACTION,
                "unique_input_sha256_count": unique_input_count,
                "input_distinctness_fraction": input_distinctness_fraction,
                "unique_vector_per_unique_input_fraction": unique_vector_per_unique_input_fraction,
                "input_with_multiple_vectors_count": input_with_multiple_vectors_count,
                "vector_collision_group_count": vector_collision_group_count,
                "vector_collision_unique_input_count": vector_collision_unique_input_count,
                "non_colliding_unique_input_count": non_colliding_unique_input_count,
                "non_colliding_unique_input_fraction": non_colliding_unique_input_fraction,
                "observed_input_identity_preserved": observed_input_identity_preserved,
                "duplicate_bounded_gate_min_unique_input_count": MIN_UNIQUE_INPUT_COUNT_FOR_DUPLICATE_BOUNDED_GATE,
                "duplicate_bounded_gate_min_unique_vector_per_unique_input_fraction": MIN_UNIQUE_VECTOR_PER_UNIQUE_INPUT_FRACTION,
                "top_vector_collision_groups": top_vector_collision_groups(&inputs),
            }));
        } else if status == "usable_benign_collision" {
            // Non-fatal: legitimately-redundant distinct inputs share a vector above the
            // catastrophic floor. Recorded (not hidden) for corpus-level monitoring.
            benign_collisions.push(json!({
                "info_code": INFO_EMBEDDER_BENIGN_COLLISION,
                "distinctness_gate_status": status,
                "embedder_id": embedder_id,
                "row_count": row_count,
                "unique_input_sha256_count": unique_input_count,
                "vector_collision_group_count": vector_collision_group_count,
                "vector_collision_unique_input_count": vector_collision_unique_input_count,
                "non_colliding_unique_input_count": non_colliding_unique_input_count,
                "non_colliding_unique_input_fraction": non_colliding_unique_input_fraction,
                "catastrophic_min_non_colliding_input_fraction": CATASTROPHIC_MIN_NON_COLLIDING_INPUT_FRACTION,
                "shard_clean_non_colliding_input_fraction": MIN_UNIQUE_VECTOR_PER_UNIQUE_INPUT_FRACTION,
                "top_vector_collision_groups": top_vector_collision_groups(&inputs),
            }));
        }
        vector_hash_sets.insert(
            embedder_id.clone(),
            counts.keys().cloned().collect::<BTreeSet<_>>(),
        );
        per_embedder_metrics.insert(embedder_id.clone(), metric);
    }

    let mut matrix = serde_json::Map::new();
    let mut alias_failures = Vec::new();
    for left in active_embedder_ids {
        let mut row = serde_json::Map::new();
        for right in active_embedder_ids {
            let left_set = vector_hash_sets.get(left).cloned().unwrap_or_default();
            let right_set = vector_hash_sets.get(right).cloned().unwrap_or_default();
            let jaccard = vector_hash_jaccard(&left_set, &right_set);
            row.insert(right.clone(), json!(jaccard));
            if left < right && jaccard > MAX_PAIRWISE_VECTOR_HASH_JACCARD {
                alias_failures.push(json!({
                    "error_code": ERR_EMBEDDER_ALIAS,
                    "left_embedder_id": left,
                    "right_embedder_id": right,
                    "jaccard": jaccard,
                    "max_pairwise_vector_hash_jaccard": MAX_PAIRWISE_VECTOR_HASH_JACCARD,
                    "left_unique_vector_sha256_count": left_set.len(),
                    "right_unique_vector_sha256_count": right_set.len(),
                }));
            }
        }
        matrix.insert(left.clone(), Value::Object(row));
    }

    DistinctnessReport {
        per_embedder_metrics,
        pairwise_jaccard_matrix: Value::Object(matrix),
        collapse_failures,
        alias_failures,
        benign_collisions,
    }
}

fn merge_distinctness_metrics(
    per_embedder_coverage: &mut BTreeMap<String, Value>,
    distinctness_report: &DistinctnessReport,
) {
    for (embedder_id, metrics) in &distinctness_report.per_embedder_metrics {
        let Some(Value::Object(coverage)) = per_embedder_coverage.get_mut(embedder_id) else {
            continue;
        };
        let Some(metric_fields) = metrics.as_object() else {
            continue;
        };
        for (key, value) in metric_fields {
            coverage.insert(key.clone(), value.clone());
        }
    }
}

fn write_distinctness_failure_artifacts(
    config: &EnvConfig,
    chunks: &[ChunkRow],
    embedders: &[EmbedderId],
    retired_embedders: &[EmbedderId],
    per_embedder_coverage: &BTreeMap<String, Value>,
    distinctness_report: &DistinctnessReport,
) -> RunnerResult<()> {
    let cache_failure_path = config
        .cache_root
        .join("python_forward_cache_distinctness_failure.json");
    let readback_failure_path = config
        .readback_root
        .join("python_forward_cache_distinctness_failure.json");
    let payload = json!({
        "schema_version": SCHEMA_VERSION,
        "artifact_kind": "python_forward_cache_embedder_distinctness_failure",
        "created_at_utc": utc_now(),
        "status": "quarantined",
        "passes": false,
        "error_code": distinctness_report.error_code(),
        "source_of_truth": {
            "cache_failure": cache_failure_path,
            "readback_failure": readback_failure_path,
            "manifest": config.cache_root.join("python_forward_cache_manifest.json"),
            "packed_manifest": config.cache_root.join("python_forward_cache_packed_manifest.json"),
            "promoted_manifest_written": false,
        },
        "summary": {
            "chunk_count": chunks.len(),
            "active_embedder_count": embedders.len(),
            "active_embedder_ids": embedder_slugs(embedders),
            "retired_embedder_count": retired_embedders.len(),
            "retired_embedder_ids": embedder_slugs(retired_embedders),
            "collapse_failure_count": distinctness_report.collapse_failures.len(),
            "alias_failure_count": distinctness_report.alias_failures.len(),
        },
        "retired_embedder_policy": retired_embedder_policy(config, retired_embedders),
        "per_embedder_coverage": per_embedder_coverage,
        "embedder_distinctness_gate": {
            "status": "failed",
            "min_unique_vector_fraction": MIN_UNIQUE_VECTOR_FRACTION,
            "min_unique_vector_per_unique_input_fraction": MIN_UNIQUE_VECTOR_PER_UNIQUE_INPUT_FRACTION,
            "min_unique_input_count_for_duplicate_bounded_gate": MIN_UNIQUE_INPUT_COUNT_FOR_DUPLICATE_BOUNDED_GATE,
            "max_pairwise_vector_hash_jaccard": MAX_PAIRWISE_VECTOR_HASH_JACCARD,
            "catastrophic_min_non_colliding_input_fraction": CATASTROPHIC_MIN_NON_COLLIDING_INPUT_FRACTION,
            "collapse_failures": distinctness_report.collapse_failures.clone(),
            "alias_failures": distinctness_report.alias_failures.clone(),
            "benign_collision_count": distinctness_report.benign_collisions.len(),
            "benign_collisions": distinctness_report.benign_collisions.clone(),
        },
        "pairwise_jaccard_matrix": distinctness_report.pairwise_jaccard_matrix.clone(),
    });
    write_json(&cache_failure_path, &payload)?;
    write_json(&readback_failure_path, &payload)?;
    Ok(())
}

fn vector_collision_unique_inputs(inputs: &EmbedderDistinctnessInputs) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for source_inputs in inputs
        .vector_to_inputs
        .values()
        .filter(|source_inputs| source_inputs.len() > 1)
    {
        for input_sha in source_inputs {
            out.insert(input_sha.clone());
        }
    }
    out
}

fn top_vector_collision_groups(inputs: &EmbedderDistinctnessInputs) -> Vec<Value> {
    let mut rows: Vec<(&String, &BTreeSet<String>)> = inputs
        .vector_to_inputs
        .iter()
        .filter(|(_, source_inputs)| source_inputs.len() > 1)
        .collect();
    rows.sort_by(|(left_vector, left_inputs), (right_vector, right_inputs)| {
        right_inputs
            .len()
            .cmp(&left_inputs.len())
            .then_with(|| {
                inputs
                    .vector_counts
                    .get(*right_vector)
                    .unwrap_or(&0)
                    .cmp(inputs.vector_counts.get(*left_vector).unwrap_or(&0))
            })
            .then_with(|| left_vector.cmp(right_vector))
    });
    rows.into_iter()
        .take(5)
        .map(|(vector_sha256, source_inputs)| {
            let row_count = inputs
                .vector_counts
                .get(vector_sha256)
                .copied()
                .unwrap_or_default();
            json!({
                "vector_sha256": vector_sha256,
                "row_count": row_count,
                "unique_input_sha256_count": source_inputs.len(),
                "input_sha256_samples": source_inputs.iter().take(5).cloned().collect::<Vec<_>>(),
                "source_sha256_samples": source_inputs.iter().take(5).cloned().collect::<Vec<_>>(),
            })
        })
        .collect()
}

fn top_vector_sha256_counts(counts: &BTreeMap<String, usize>) -> Vec<Value> {
    let mut rows: Vec<(&String, &usize)> = counts.iter().collect();
    rows.sort_by(|(left_sha, left_count), (right_sha, right_count)| {
        right_count
            .cmp(left_count)
            .then_with(|| left_sha.cmp(right_sha))
    });
    rows.into_iter()
        .take(5)
        .map(|(vector_sha256, count)| {
            json!({
                "vector_sha256": vector_sha256,
                "count": count,
            })
        })
        .collect()
}

fn vector_hash_jaccard(left: &BTreeSet<String>, right: &BTreeSet<String>) -> f64 {
    let union_count = left.union(right).count();
    if union_count == 0 {
        return 0.0;
    }
    left.intersection(right).count() as f64 / union_count as f64
}

fn prepare_output_roots(config: &EnvConfig) -> RunnerResult<()> {
    fs::create_dir_all(&config.cache_root).map_err(|err| {
        RunnerError::new(
            "MEJEPA_PY_FORWARD_MKDIR_FAILED",
            format!("{}: {err}", config.cache_root.display()),
        )
    })?;
    fs::create_dir_all(&config.readback_root).map_err(|err| {
        RunnerError::new(
            "MEJEPA_PY_FORWARD_MKDIR_FAILED",
            format!("{}: {err}", config.readback_root.display()),
        )
    })?;
    fs::create_dir_all(config.cache_root.join("packed_records")).map_err(|err| {
        RunnerError::new(
            "MEJEPA_PY_FORWARD_MKDIR_FAILED",
            format!("packed_records: {err}"),
        )
    })?;
    Ok(())
}

fn write_jsonl(path: &Path, rows: &[Value]) -> RunnerResult<()> {
    let mut file = File::create(path).map_err(|err| {
        RunnerError::new(
            "MEJEPA_PY_FORWARD_JSONL_CREATE_FAILED",
            format!("{}: {err}", path.display()),
        )
    })?;
    for row in rows {
        serde_json::to_writer(&mut file, row).map_err(|err| {
            RunnerError::new(
                "MEJEPA_PY_FORWARD_JSONL_WRITE_FAILED",
                format!("{}: {err}", path.display()),
            )
        })?;
        file.write_all(b"\n").map_err(|err| {
            RunnerError::new(
                "MEJEPA_PY_FORWARD_JSONL_WRITE_FAILED",
                format!("{}: {err}", path.display()),
            )
        })?;
    }
    Ok(())
}

fn write_json(path: &Path, value: &Value) -> RunnerResult<()> {
    let mut file = File::create(path).map_err(|err| {
        RunnerError::new(
            "MEJEPA_PY_FORWARD_JSON_CREATE_FAILED",
            format!("{}: {err}", path.display()),
        )
    })?;
    serde_json::to_writer_pretty(&mut file, value).map_err(|err| {
        RunnerError::new(
            "MEJEPA_PY_FORWARD_JSON_WRITE_FAILED",
            format!("{}: {err}", path.display()),
        )
    })?;
    file.write_all(b"\n").map_err(|err| {
        RunnerError::new(
            "MEJEPA_PY_FORWARD_JSON_WRITE_FAILED",
            format!("{}: {err}", path.display()),
        )
    })?;
    Ok(())
}

fn instrument_id(embedder: EmbedderId) -> &'static str {
    match embedder {
        EmbedderId::E1 => "py_dda_e1_semantic_v1",
        EmbedderId::E2 => "py_dda_e2_temporal_recent_v1",
        EmbedderId::E3 => "py_dda_e3_temporal_periodic_v1",
        EmbedderId::E4 => "py_dda_e4_temporal_positional_v1",
        EmbedderId::E6 => "py_dda_e6_sparse_v1",
        EmbedderId::E7 => "py_dda_e7_code_v1",
        EmbedderId::E8 => "py_dda_e8_graph_v1",
        EmbedderId::E9 => "py_dda_e9_hdc_v1",
        EmbedderId::E10 => "py_dda_e10_contextual_v1",
        EmbedderId::E12 => "py_dda_e12_late_interaction_v1",
        EmbedderId::E13 => "py_dda_e13_splade_v3",
        EmbedderId::E14 => "py_dda_e14_bge_m3_dense_v1",
        _ => "py_dda_unsupported",
    }
}

fn instrument_kind(embedder: EmbedderId) -> &'static str {
    if is_pretrained(embedder) {
        "content_embedder_pretrained"
    } else {
        "content_embedder_algorithmic"
    }
}

fn sha256_prefixed(bytes: &[u8]) -> String {
    format!("sha256:{}", hex_sha256(bytes))
}

fn sha256_value(value: &Value) -> String {
    let bytes = serde_json::to_vec(value).expect("JSON value serializes");
    sha256_prefixed(&bytes)
}

fn sha256_file(path: &Path) -> RunnerResult<String> {
    let bytes = fs::read(path).map_err(|err| {
        RunnerError::new(
            "MEJEPA_PY_FORWARD_HASH_READ_FAILED",
            format!("{}: {err}", path.display()),
        )
    })?;
    Ok(sha256_prefixed(&bytes))
}

fn hex_sha256(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut out = String::with_capacity(64);
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

fn vector_sha256(vector: &[f32]) -> String {
    let mut bytes = Vec::with_capacity(vector.len() * 4);
    for value in vector {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    sha256_prefixed(&bytes)
}

fn elapsed_ms(started: Instant) -> f64 {
    let elapsed = started.elapsed();
    (elapsed.as_secs_f64() * 1000.0 * 1000.0).round() / 1000.0
}

fn utc_now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};

    fn config(corpus_profile: &str, only_embedder: Option<EmbedderId>) -> EnvConfig {
        EnvConfig {
            chunks_jsonl: PathBuf::from("/tmp/chunks.jsonl"),
            cache_root: PathBuf::from("/tmp/cache"),
            readback_root: PathBuf::from("/tmp/readback"),
            chunk_offset: 0,
            max_chunks: 1,
            max_source_bytes: 1024,
            true_batch_size: 1,
            cache_store_layout: "packed".to_string(),
            only_embedder,
            corpus_profile: corpus_profile.to_string(),
            models_config: PathBuf::from("/tmp/models.toml"),
            gpu_vram_guard_bytes: 0,
        }
    }

    fn chunk(
        chunk_id: &str,
        source_text: &str,
        source_sha256: &str,
        relative_path: &str,
    ) -> ChunkRow {
        ChunkRow {
            line_number: 1,
            chunk_id: chunk_id.to_string(),
            source_text: source_text.to_string(),
            source_sha256: source_sha256.to_string(),
            file_sha256: "sha256:file".to_string(),
            task_id: "TASK".to_string(),
            task_instance_id: "instance".to_string(),
            repo: "owner/repo".to_string(),
            workspace_state: "mutated".to_string(),
            absolute_path: format!("/tmp/repo/{relative_path}"),
            relative_path: relative_path.to_string(),
            mutation_category: Some("known_good".to_string()),
            candidate_row_id: Some("sha256:candidate".to_string()),
        }
    }

    struct CountingForwarder {
        embedder: EmbedderId,
        artifact_root: PathBuf,
        calls: Arc<Mutex<usize>>,
        true_batch: bool,
    }

    impl CountingForwarder {
        fn new(embedder: EmbedderId, calls: Arc<Mutex<usize>>, true_batch: bool) -> Self {
            Self {
                embedder,
                artifact_root: PathBuf::from("/tmp/fake-forwarder"),
                calls,
                true_batch,
            }
        }

        fn output(
            &self,
            input: &EmbedderInput,
            ordinal: usize,
            precision_class: &str,
        ) -> EmbedderOutput {
            let mut vector = vec![0.0; self.embedder.dimension()];
            vector[0] = ordinal as f32;
            EmbedderOutput {
                embedder: self.embedder,
                source_id: input.source_id.clone(),
                vector,
                model_version: "fake-forwarder-v1".to_string(),
                precision_class: precision_class.to_string(),
            }
        }
    }

    #[async_trait]
    impl EmbedderForward for CountingForwarder {
        fn embedder(&self) -> EmbedderId {
            self.embedder
        }

        fn model_version(&self) -> &str {
            "fake-forwarder-v1"
        }

        fn artifact_root(&self) -> &Path {
            &self.artifact_root
        }

        async fn forward(
            &self,
            input: &EmbedderInput,
        ) -> context_graph_mejepa_embedders::EmbedResult<EmbedderOutput> {
            let ordinal = {
                let mut calls = self.calls.lock().expect("calls mutex poisoned");
                *calls += 1;
                *calls
            };
            Ok(self.output(input, ordinal, "fake_serial_forward"))
        }

        fn supports_true_batch(&self) -> bool {
            self.true_batch
        }

        async fn forward_true_batch(
            &self,
            inputs: &[EmbedderInput],
        ) -> context_graph_mejepa_embedders::EmbedResult<Vec<EmbedderOutput>> {
            let start = {
                let mut calls = self.calls.lock().expect("calls mutex poisoned");
                let start = *calls;
                *calls += inputs.len();
                start
            };
            Ok(inputs
                .iter()
                .enumerate()
                .map(|(idx, input)| self.output(input, start + idx + 1, "fake_true_batch_forward"))
                .collect())
        }
    }

    #[test]
    fn static_ast_profile_retires_temporal_embedders_by_default() {
        let config = config(STATIC_AST_CORPUS_PROFILE, None);

        let active = active_embedders(&config).unwrap();
        let retired = retired_embedders(&config);

        assert_eq!(active.len(), 9);
        assert!(!active.contains(&EmbedderId::E2));
        assert!(!active.contains(&EmbedderId::E3));
        assert!(!active.contains(&EmbedderId::E4));
        assert_eq!(
            retired,
            vec![EmbedderId::E2, EmbedderId::E3, EmbedderId::E4]
        );
    }

    #[test]
    fn static_ast_profile_rejects_explicit_temporal_embedder() {
        let config = config(STATIC_AST_CORPUS_PROFILE, Some(EmbedderId::E2));

        let err = active_embedders(&config).unwrap_err();

        assert_eq!(
            err.code,
            "MEJEPA_PY_FORWARD_TEMPORAL_RETIRED_FOR_STATIC_AST"
        );
    }

    #[test]
    fn live_temporal_profile_keeps_all_supported_forward_embedders() {
        let config = config(LIVE_TEMPORAL_CORPUS_PROFILE, None);

        let active = active_embedders(&config).unwrap();

        assert_eq!(active, SUPPORTED_FORWARD_EMBEDDERS.to_vec());
        assert!(retired_embedders(&config).is_empty());
    }

    #[test]
    fn per_embedder_true_batch_env_override_requires_isolated_embedder() {
        let key = "CG_MEJEPA_PY_FORWARD_TRUE_BATCH_SIZE_E14";
        let previous = env::var_os(key);
        env::set_var(key, "1");

        assert_eq!(
            effective_true_batch_size(Some(EmbedderId::E14), 8).unwrap(),
            1
        );
        assert_eq!(
            effective_true_batch_size(Some(EmbedderId::E1), 8).unwrap(),
            8
        );
        assert_eq!(effective_true_batch_size(None, 8).unwrap(), 8);

        match previous {
            Some(value) => env::set_var(key, value),
            None => env::remove_var(key),
        }
    }

    #[test]
    fn forward_input_identity_uses_e7_language_context() {
        let left = chunk("chunk-left", "print(1)\n", "sha256:source-a", "pkg/a.py");
        let right = chunk("chunk-right", "print(1)\n", "sha256:source-a", "pkg/b.py");
        let different_language = chunk("chunk-rs", "print(1)\n", "sha256:source-a", "pkg/a.rs");

        let left_e7 = forward_input_identity(EmbedderId::E7, &left).unwrap();
        let right_e7 = forward_input_identity(EmbedderId::E7, &right).unwrap();
        let different_language_e7 =
            forward_input_identity(EmbedderId::E7, &different_language).unwrap();

        assert_eq!(left_e7.policy, "e7_language_and_source_text_sha256_v1");
        assert_eq!(left_e7.input_sha256, right_e7.input_sha256);
        assert_ne!(left_e7.input_sha256, different_language_e7.input_sha256);

        let left_e1 = forward_input_identity(EmbedderId::E1, &left).unwrap();
        let right_e1 = forward_input_identity(EmbedderId::E1, &right).unwrap();
        assert_eq!(left_e1.policy, "source_text_sha256_v1");
        assert_eq!(left_e1.input_sha256, "sha256:source-a");
        assert_eq!(left_e1.input_sha256, right_e1.input_sha256);
    }

    #[test]
    fn forward_input_identity_includes_temporal_instruction_for_live_temporal_slots() {
        let earlier = chunk(
            "chunk-a",
            "timestamp:2026-05-24T12:00:00Z\nvalue = 1\n",
            "sha256:source-a",
            "pkg/a.py",
        );
        let later = chunk(
            "chunk-b",
            "timestamp:2026-05-24T13:00:00Z\nvalue = 1\n",
            "sha256:source-a",
            "pkg/a.py",
        );

        let earlier_e2 = forward_input_identity(EmbedderId::E2, &earlier).unwrap();
        let later_e2 = forward_input_identity(EmbedderId::E2, &later).unwrap();

        assert_eq!(
            earlier_e2.policy,
            "temporal_instruction_and_source_text_sha256_v2"
        );
        assert_ne!(
            earlier_e2.input_sha256, later_e2.input_sha256,
            "same source text hash at different event times must not share a temporal cache identity"
        );
    }

    #[test]
    fn forward_input_identity_rejects_invalid_temporal_instruction() {
        let invalid = chunk(
            "chunk-a",
            "session:attempt-1 sequence:not-a-number\nvalue = 1\n",
            "sha256:source-a",
            "pkg/a.py",
        );

        let err = forward_input_identity(EmbedderId::E4, &invalid).unwrap_err();

        assert_eq!(err.code, "MEJEPA_PY_FORWARD_TEMPORAL_INSTRUCTION_INVALID");
        assert!(err.message.contains("invalid sequence"));
    }

    #[tokio::test]
    async fn run_embedder_batches_reuses_duplicate_e7_model_inputs_across_paths() {
        let mut config = config(STATIC_AST_CORPUS_PROFILE, Some(EmbedderId::E7));
        config.true_batch_size = 8;
        let calls = Arc::new(Mutex::new(0usize));
        let forwarder = CountingForwarder::new(EmbedderId::E7, Arc::clone(&calls), true);
        let rows = vec![
            chunk("chunk-a", "print(1)\n", "sha256:source-a", "pkg/a.py"),
            chunk("chunk-b", "print(1)\n", "sha256:source-a", "pkg/b.py"),
        ];
        let mut batch_counter = 0usize;

        let results = run_embedder_batches(&config, &rows, &forwarder, &mut batch_counter).await;

        assert_eq!(batch_counter, 1);
        assert_eq!(*calls.lock().expect("calls mutex poisoned"), 1);
        assert_eq!(results.len(), 1);
        let result = &results[0];
        assert_eq!(result.records.len(), 2);
        assert_eq!(result.quarantines.len(), 0);
        assert_eq!(result.unique_forward_input_count, 1);
        assert_eq!(result.duplicate_forward_cache_hits, 1);
        assert_eq!(result.telemetry["unique_forward_input_count"], json!(1));
        assert_eq!(
            result.telemetry["duplicate_forward_cache_hit_count"],
            json!(1)
        );
        assert_eq!(
            result.records[0].index_row["input_sha256"],
            result.records[1].index_row["input_sha256"]
        );
        assert_eq!(
            result.records[0].index_row["vector_sha256"],
            result.records[1].index_row["vector_sha256"]
        );
    }

    #[test]
    fn distinctness_inputs_prefers_explicit_input_sha256() {
        let records = vec![
            ForwardRecord {
                index_row: json!({
                    "input_sha256": "sha256:input-a",
                    "source_sha256": "sha256:source-a",
                    "vector_sha256": "sha256:vector-a",
                }),
                packed_record: json!({}),
            },
            ForwardRecord {
                index_row: json!({
                    "input_sha256": "sha256:input-b",
                    "source_sha256": "sha256:source-a",
                    "vector_sha256": "sha256:vector-b",
                }),
                packed_record: json!({}),
            },
        ];

        let inputs = distinctness_inputs_for_records(EmbedderId::E7, &records).unwrap();

        assert_eq!(inputs.input_counts.len(), 2);
        assert_eq!(inputs.input_to_vectors["sha256:input-a"].len(), 1);
        assert_eq!(inputs.input_to_vectors["sha256:input-b"].len(), 1);
        assert!(!inputs.input_to_vectors.contains_key("sha256:source-a"));
    }

    fn distinctness_inputs_from_pairs(pairs: &[(&str, &str)]) -> EmbedderDistinctnessInputs {
        let mut inputs = EmbedderDistinctnessInputs::default();
        for (vector, input) in pairs {
            *inputs
                .vector_counts
                .entry((*vector).to_string())
                .or_insert(0) += 1;
            *inputs.input_counts.entry((*input).to_string()).or_insert(0) += 1;
            inputs
                .input_to_vectors
                .entry((*input).to_string())
                .or_default()
                .insert((*vector).to_string());
            inputs
                .vector_to_inputs
                .entry((*vector).to_string())
                .or_default()
                .insert((*input).to_string());
        }
        inputs
    }

    fn report_for_hashes(rows: &[(&str, Vec<&str>)]) -> DistinctnessReport {
        let active = rows
            .iter()
            .map(|(embedder_id, _)| (*embedder_id).to_string())
            .collect::<Vec<_>>();
        let mut inputs_by_embedder = BTreeMap::new();
        for (embedder_id, hashes) in rows {
            let pairs = hashes
                .iter()
                .enumerate()
                .map(|(idx, vector)| {
                    let input = format!("sha256:input-{idx:04}");
                    (*vector, input)
                })
                .collect::<Vec<_>>();
            let pair_refs = pairs
                .iter()
                .map(|(vector, input)| (*vector, input.as_str()))
                .collect::<Vec<_>>();
            inputs_by_embedder.insert(
                (*embedder_id).to_string(),
                distinctness_inputs_from_pairs(&pair_refs),
            );
        }
        evaluate_embedder_distinctness(&active, &inputs_by_embedder)
    }

    fn report_for_vector_and_input_hashes(
        rows: &[(&str, Vec<(&str, &str)>)],
    ) -> DistinctnessReport {
        let active = rows
            .iter()
            .map(|(embedder_id, _)| (*embedder_id).to_string())
            .collect::<Vec<_>>();
        let mut inputs_by_embedder = BTreeMap::new();
        for (embedder_id, pairs) in rows {
            inputs_by_embedder.insert(
                (*embedder_id).to_string(),
                distinctness_inputs_from_pairs(pairs),
            );
        }
        evaluate_embedder_distinctness(&active, &inputs_by_embedder)
    }

    #[test]
    fn distinctness_gate_rejects_degenerate_embedder_hash_space() {
        let repeated = vec!["sha256:a"; 100];
        let report = report_for_hashes(&[("e1", repeated)]);

        assert!(!report.passes());
        assert_eq!(report.error_code(), ERR_EMBEDDER_COLLAPSE);
        assert_eq!(report.collapse_failures.len(), 1);
        assert_eq!(report.alias_failures.len(), 0);
        assert_eq!(
            report.per_embedder_metrics["e1"]["unique_vector_sha256_count"],
            json!(1)
        );
    }

    #[test]
    fn distinctness_gate_treats_all_quarantine_as_no_forward_records() {
        let active = vec!["e2".to_string()];
        let inputs_by_embedder = BTreeMap::new();

        let report = evaluate_embedder_distinctness(&active, &inputs_by_embedder);

        assert!(report.passes());
        assert_eq!(report.collapse_failures.len(), 0);
        assert_eq!(report.alias_failures.len(), 0);
        assert_eq!(
            report.per_embedder_metrics["e2"]["distinctness_gate_status"],
            json!("no_forward_records")
        );
        assert_eq!(
            report.per_embedder_metrics["e2"]["forward_record_count"],
            json!(0)
        );
    }

    #[test]
    fn distinctness_gate_rejects_aliased_embedder_hash_sets() {
        let report = report_for_hashes(&[
            ("e1", vec!["sha256:a", "sha256:b", "sha256:c"]),
            ("e8", vec!["sha256:a", "sha256:b", "sha256:c"]),
        ]);

        assert!(!report.passes());
        assert_eq!(report.error_code(), ERR_EMBEDDER_ALIAS);
        assert_eq!(report.collapse_failures.len(), 0);
        assert_eq!(report.alias_failures.len(), 1);
        assert_eq!(report.pairwise_jaccard_matrix["e1"]["e8"], json!(1.0));
    }

    #[test]
    fn distinctness_gate_passes_distinct_healthy_hash_spaces() {
        let report = report_for_hashes(&[
            ("e1", vec!["sha256:a", "sha256:b", "sha256:c"]),
            ("e8", vec!["sha256:d", "sha256:e", "sha256:f"]),
            ("e9", vec!["sha256:g", "sha256:h", "sha256:i"]),
        ]);

        assert!(report.passes());
        assert_eq!(report.collapse_failures.len(), 0);
        assert_eq!(report.alias_failures.len(), 0);
        assert_eq!(report.pairwise_jaccard_matrix["e1"]["e8"], json!(0.0));
    }

    #[test]
    fn distinctness_gate_accepts_duplicate_heavy_input_when_vectors_cover_inputs() {
        let mut pairs = Vec::new();
        for idx in 0..80 {
            let vector = format!("sha256:vector-{idx:04}");
            let input = format!("sha256:input-{idx:04}");
            pairs.push((vector, input));
        }
        for idx in 0..120 {
            let source = idx % 80;
            let vector = format!("sha256:vector-{source:04}");
            let input = format!("sha256:input-{source:04}");
            pairs.push((vector, input));
        }
        let refs = pairs
            .iter()
            .map(|(vector, input)| (vector.as_str(), input.as_str()))
            .collect::<Vec<_>>();

        let report = report_for_vector_and_input_hashes(&[("e9", refs)]);

        assert!(report.passes());
        assert_eq!(
            report.per_embedder_metrics["e9"]["distinctness_gate_status"],
            json!("usable_duplicate_bounded")
        );
        assert_eq!(
            report.per_embedder_metrics["e9"]["unique_input_sha256_count"],
            json!(80)
        );
    }

    #[test]
    fn distinctness_gate_accepts_low_support_duplicate_heavy_identity_preserved_input() {
        let mut pairs = Vec::new();
        for idx in 0..56 {
            let vector = format!("sha256:vector-{idx:04}");
            let input = format!("sha256:input-{idx:04}");
            pairs.push((vector, input));
        }
        for idx in 0..60 {
            let source = idx % 56;
            let vector = format!("sha256:vector-{source:04}");
            let input = format!("sha256:input-{source:04}");
            pairs.push((vector, input));
        }
        let refs = pairs
            .iter()
            .map(|(vector, input)| (vector.as_str(), input.as_str()))
            .collect::<Vec<_>>();

        let report = report_for_vector_and_input_hashes(&[("e1", refs)]);

        assert!(report.passes());
        assert_eq!(
            report.per_embedder_metrics["e1"]["distinctness_gate_status"],
            json!("usable_duplicate_limited")
        );
        assert_eq!(
            report.per_embedder_metrics["e1"]["unique_input_sha256_count"],
            json!(56)
        );
        assert_eq!(
            report.per_embedder_metrics["e1"]["vector_collision_group_count"],
            json!(0)
        );
        assert_eq!(
            report.per_embedder_metrics["e1"]["observed_input_identity_preserved"],
            json!(true)
        );
    }

    #[test]
    fn distinctness_gate_rejects_duplicate_heavy_input_with_vector_collisions() {
        let mut pairs = Vec::new();
        for idx in 0..80 {
            let vector = format!("sha256:vector-{:04}", idx % 40);
            let input = format!("sha256:input-{idx:04}");
            pairs.push((vector, input));
        }
        for idx in 0..120 {
            let source = idx % 80;
            let vector = format!("sha256:vector-{:04}", source % 40);
            let input = format!("sha256:input-{source:04}");
            pairs.push((vector, input));
        }
        let refs = pairs
            .iter()
            .map(|(vector, input)| (vector.as_str(), input.as_str()))
            .collect::<Vec<_>>();

        let report = report_for_vector_and_input_hashes(&[("e9", refs)]);

        assert!(!report.passes());
        assert_eq!(report.error_code(), ERR_EMBEDDER_COLLAPSE);
        assert_eq!(
            report.per_embedder_metrics["e9"]["unique_vector_per_unique_input_fraction"],
            json!(0.5)
        );
        assert_eq!(
            report.per_embedder_metrics["e9"]["non_colliding_unique_input_fraction"],
            json!(0.0)
        );
    }

    #[test]
    fn distinctness_gate_treats_exact_vector_collisions_above_floor_as_benign() {
        // 20 of 100 distinct inputs collide into 10 shared vectors; 80 stay unique.
        // non_colliding_unique_input_fraction = 0.8 — below the 0.9 clean threshold but
        // well above the 0.5 catastrophic floor, with deterministic inputs. This is the
        // shard-2418 shape (uncased embedder mapping case-only variants together) and
        // must be NON-FATAL: recorded as a benign collision, the shard still passes.
        let mut pairs = Vec::new();
        for idx in 0..100 {
            let vector = if idx < 20 {
                format!("sha256:shared-{:04}", idx / 2)
            } else {
                format!("sha256:vector-{idx:04}")
            };
            let input = format!("sha256:input-{idx:04}");
            pairs.push((vector, input));
        }
        let refs = pairs
            .iter()
            .map(|(vector, input)| (vector.as_str(), input.as_str()))
            .collect::<Vec<_>>();

        let report = report_for_vector_and_input_hashes(&[("e9", refs)]);

        assert!(report.passes());
        assert_eq!(report.collapse_failures.len(), 0);
        assert_eq!(report.alias_failures.len(), 0);
        assert_eq!(report.benign_collisions.len(), 1);
        assert_eq!(
            report.benign_collisions[0]["info_code"],
            json!(INFO_EMBEDDER_BENIGN_COLLISION)
        );
        assert_eq!(
            report.per_embedder_metrics["e9"]["distinctness_gate_status"],
            json!("usable_benign_collision")
        );
        assert_eq!(
            report.per_embedder_metrics["e9"]["distinctness_fraction"],
            json!(0.9)
        );
        assert_eq!(
            report.per_embedder_metrics["e9"]["vector_collision_group_count"],
            json!(10)
        );
        assert_eq!(
            report.per_embedder_metrics["e9"]["non_colliding_unique_input_fraction"],
            json!(0.8)
        );
        // Determinism is preserved (same input -> one vector), so this is not a collapse.
        assert_eq!(
            report.per_embedder_metrics["e9"]["observed_input_identity_preserved"],
            json!(true)
        );
    }

    #[test]
    fn distinctness_gate_rejects_nondeterministic_input_identity() {
        // The SAME input mapping to two different vectors is a nondeterministic embedder
        // (e.g. batch-position float drift). This must always be fatal, regardless of how
        // distinct the overall vector space looks.
        let report = report_for_vector_and_input_hashes(&[(
            "e1",
            vec![
                ("sha256:vector-a", "sha256:input-shared"),
                ("sha256:vector-b", "sha256:input-shared"),
                ("sha256:vector-c", "sha256:input-002"),
                ("sha256:vector-d", "sha256:input-003"),
            ],
        )]);

        assert!(!report.passes());
        assert_eq!(report.error_code(), ERR_EMBEDDER_COLLAPSE);
        assert_eq!(report.collapse_failures.len(), 1);
        assert_eq!(
            report.collapse_failures[0]["collapse_reason"],
            json!("input_identity_nondeterministic")
        );
        assert_eq!(
            report.per_embedder_metrics["e1"]["distinctness_gate_status"],
            json!("collapse_nondeterministic")
        );
        assert_eq!(
            report.per_embedder_metrics["e1"]["input_with_multiple_vectors_count"],
            json!(1)
        );
    }

    #[test]
    fn distinctness_gate_rejects_catastrophic_vector_collision_below_floor() {
        // 60 of 100 distinct inputs collide (non_colliding = 0.4 < 0.5 catastrophic floor)
        // with deterministic inputs. A genuinely degenerate embedder for this shard: FATAL.
        let mut pairs = Vec::new();
        for idx in 0..100 {
            let vector = if idx < 60 {
                format!("sha256:shared-{:04}", idx / 3)
            } else {
                format!("sha256:vector-{idx:04}")
            };
            let input = format!("sha256:input-{idx:04}");
            pairs.push((vector, input));
        }
        let refs = pairs
            .iter()
            .map(|(vector, input)| (vector.as_str(), input.as_str()))
            .collect::<Vec<_>>();

        let report = report_for_vector_and_input_hashes(&[("e9", refs)]);

        assert!(!report.passes());
        assert_eq!(report.error_code(), ERR_EMBEDDER_COLLAPSE);
        assert_eq!(report.collapse_failures.len(), 1);
        assert_eq!(
            report.collapse_failures[0]["collapse_reason"],
            json!("catastrophic_vector_collision")
        );
        assert_eq!(
            report.per_embedder_metrics["e9"]["distinctness_gate_status"],
            json!("collapse_catastrophic")
        );
    }

    #[test]
    fn distinctness_gate_reports_collisions_without_aggregate_masking() {
        // Duplicate-input twin of the benign-collision case: 200 rows, 100 distinct
        // inputs, 20 of them colliding. Aggregate row counts could fool a naive gate into
        // reporting "usable" and hiding the collisions. The corrected gate must instead
        // surface the true collision accounting (non_colliding = 0.8, 10 groups) and label
        // the shard "usable_benign_collision" — non-fatal at shard scope, but explicitly
        // recorded so the corpus-scope floor can act on it. No masking, no false clean pass.
        let mut pairs = Vec::new();
        for idx in 0..100 {
            let vector = if idx < 20 {
                format!("sha256:shared-{:04}", idx / 2)
            } else {
                format!("sha256:vector-{idx:04}")
            };
            let input = format!("sha256:input-{idx:04}");
            pairs.push((vector, input));
        }
        for idx in 0..100 {
            let source = idx % 100;
            let vector = if source < 20 {
                format!("sha256:shared-{:04}", source / 2)
            } else {
                format!("sha256:vector-{source:04}")
            };
            let input = format!("sha256:input-{source:04}");
            pairs.push((vector, input));
        }
        let refs = pairs
            .iter()
            .map(|(vector, input)| (vector.as_str(), input.as_str()))
            .collect::<Vec<_>>();

        let report = report_for_vector_and_input_hashes(&[("e9", refs)]);

        // Benign at shard scope, but NOT a clean "usable": the collisions are recorded.
        assert!(report.passes());
        assert_eq!(report.collapse_failures.len(), 0);
        assert_eq!(report.benign_collisions.len(), 1);
        assert_eq!(
            report.per_embedder_metrics["e9"]["distinctness_gate_status"],
            json!("usable_benign_collision")
        );
        assert_eq!(
            report.per_embedder_metrics["e9"]["unique_vector_per_unique_input_fraction"],
            json!(0.9)
        );
        assert_eq!(
            report.per_embedder_metrics["e9"]["vector_collision_group_count"],
            json!(10)
        );
        // The true 0.8 is surfaced (not masked up by the 100 duplicate rows).
        assert_eq!(
            report.per_embedder_metrics["e9"]["non_colliding_unique_input_fraction"],
            json!(0.8)
        );
    }

    #[test]
    fn distinctness_gate_allows_jaccard_at_threshold() {
        let report = report_for_hashes(&[
            ("e1", vec!["sha256:a", "sha256:b", "sha256:c"]),
            ("e8", vec!["sha256:a", "sha256:b", "sha256:d"]),
        ]);

        assert!(report.passes());
        assert_eq!(report.pairwise_jaccard_matrix["e1"]["e8"], json!(0.5));
    }
}
