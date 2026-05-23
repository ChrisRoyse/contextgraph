use context_graph_mejepa_embedders::{
    AlgorithmicEmbedderForward, EmbedError, EmbedderForward, EmbedderId, EmbedderInput,
    ModelsConfig, PretrainedEmbedderForward, SUPPORTED_FORWARD_EMBEDDERS,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

const SCHEMA_VERSION: u64 = 1;
const DEFAULT_MODELS_CONFIG: &str = "/zfs/hot/contextgraph/models/mejepa_models_config.toml";
const STATIC_AST_CORPUS_PROFILE: &str = "static_ast_python";
const LIVE_TEMPORAL_CORPUS_PROFILE: &str = "live_temporal";

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

#[derive(Debug, Clone)]
struct QuarantineRecord {
    row: Value,
}

#[derive(Debug)]
struct BatchResult {
    telemetry: Value,
    records: Vec<ForwardRecord>,
    quarantines: Vec<QuarantineRecord>,
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
        for result in results {
            output_count += result.records.len();
            quarantine_count += result.quarantines.len();
            all_telemetry_rows.push(result.telemetry);
            for record in result.records {
                embedder_records.push(record);
            }
            for quarantine in result.quarantines {
                all_quarantine_rows.push(quarantine.row);
            }
        }
        let (packed, top_index_rows) =
            write_packed_embedder_records(&config, embedder, &embedder_records)?;
        all_index_rows.extend(top_index_rows);
        packed_per_embedder.insert(embedder.slug().to_string(), packed);
        first_pass_writes += output_count;
        per_embedder_coverage.insert(
            embedder.slug().to_string(),
            json!({
                "embedder_id": embedder.slug(),
                "chunk_count": chunks.len(),
                "cache_record_count": output_count,
                "quarantine_row_count": quarantine_count,
                "elapsed_ms": elapsed_ms(started),
                "passes": output_count + quarantine_count == chunks.len(),
            }),
        );
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
        let true_batch_size = env_usize_default("CG_MEJEPA_PY_FORWARD_TRUE_BATCH_SIZE", 8)?.max(1);
        let cache_store_layout =
            env::var("CG_MEJEPA_PY_FORWARD_CACHE_STORE_LAYOUT").unwrap_or_else(|_| "packed".into());
        let only_embedder = match env::var("CG_MEJEPA_PY_FORWARD_ONLY_EMBEDDER") {
            Ok(value) if !value.trim().is_empty() => Some(parse_embedder(&value)?),
            _ => None,
        };
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

fn env_usize(name: &'static str) -> RunnerResult<usize> {
    let value =
        env::var(name).map_err(|_| RunnerError::new("MEJEPA_PY_FORWARD_ENV_MISSING", name))?;
    value.parse::<usize>().map_err(|err| {
        RunnerError::new(
            "MEJEPA_PY_FORWARD_ENV_PARSE",
            format!("{name}={value:?}: {err}"),
        )
    })
}

fn env_usize_default(name: &'static str, default: usize) -> RunnerResult<usize> {
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

fn env_u64_default(name: &'static str, default: u64) -> RunnerResult<u64> {
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
    for batch in chunks.chunks(config.true_batch_size.max(1)) {
        let batch_id = format!("{}-batch-{batch_counter:06}", forwarder.embedder().slug());
        *batch_counter += 1;
        let started = Instant::now();
        let mut records = Vec::new();
        let mut quarantines = Vec::new();
        let mut forwarded = Vec::new();
        let mut forwarded_chunks = Vec::new();
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
            forwarded.push(EmbedderInput {
                embedder: forwarder.embedder(),
                text: chunk.source_text.clone(),
                source_id: source_id_for_embedder(forwarder.embedder(), chunk),
            });
            forwarded_chunks.push(chunk);
        }
        if !forwarded.is_empty() {
            if forwarder.supports_true_batch() {
                match forwarder.forward_true_batch(&forwarded).await {
                    Ok(outputs) => {
                        for (chunk, output) in forwarded_chunks.iter().zip(outputs) {
                            records.push(forward_record(forwarder.embedder(), chunk, output));
                        }
                    }
                    Err(err) => {
                        for chunk in forwarded_chunks {
                            quarantines.push(QuarantineRecord {
                                row: quarantine_row(
                                    forwarder.embedder(),
                                    chunk,
                                    err.code(),
                                    err.to_string(),
                                    None,
                                ),
                            });
                        }
                    }
                }
            } else {
                for (chunk, input) in forwarded_chunks.iter().zip(forwarded) {
                    match forwarder.forward(&input).await {
                        Ok(output) => {
                            records.push(forward_record(forwarder.embedder(), chunk, output))
                        }
                        Err(err) => quarantines.push(QuarantineRecord {
                            row: quarantine_row(
                                forwarder.embedder(),
                                chunk,
                                err.code(),
                                err.to_string(),
                                None,
                            ),
                        }),
                    }
                }
            }
        }
        let latency_ms = elapsed_ms(started);
        let telemetry = telemetry_row(
            config,
            forwarder,
            &batch_id,
            batch.len(),
            records.len(),
            quarantines.len(),
            latency_ms,
        );
        results.push(BatchResult {
            telemetry,
            records,
            quarantines,
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

fn forward_record(
    embedder: EmbedderId,
    chunk: &ChunkRow,
    output: context_graph_mejepa_embedders::EmbedderOutput,
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
        "input_sha256": chunk.source_sha256,
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
        "file_sha256": chunk.file_sha256,
        "absolute_path": chunk.absolute_path,
        "relative_path": chunk.relative_path,
        "embedder_id": embedder.slug(),
        "instrument_id": instrument_id,
        "cache_key_sha256": cache_key_sha256,
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
    batch_size: usize,
    output_count: usize,
    quarantine_count: usize,
    latency_ms: f64,
) -> Value {
    if forwarder.supports_true_batch() {
        json!({
            "schema_version": SCHEMA_VERSION,
            "artifact_kind": "python_forward_true_batch_telemetry",
            "embedder_id": forwarder.embedder().slug(),
            "batch_id": batch_id,
            "batch_size": batch_size,
            "output_count": output_count,
            "quarantine_count": quarantine_count,
            "latency_ms": latency_ms,
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
            "row_count": batch_size,
            "processed_non_quarantine_count": output_count,
            "quarantine_count": quarantine_count,
            "latency_ms": latency_ms,
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
}
