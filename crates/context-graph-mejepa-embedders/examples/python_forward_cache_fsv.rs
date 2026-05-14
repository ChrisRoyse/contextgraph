use context_graph_embeddings::get_memory_estimate;
use context_graph_mejepa_embedders::{
    digest_manifest_for_embedder, embedder_model_id, AlgorithmicEmbedderForward, EmbedderForward,
    EmbedderId, EmbedderInput, EmbedderOutput, EmbedderRegistration, ModelsConfig,
    PretrainedEmbedderForward, SUPPORTED_FORWARD_EMBEDDERS, SUPPORTED_PRETRAINED_FORWARD_EMBEDDERS,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

type FsvResult<T> = Result<T, Box<dyn std::error::Error>>;

const DEFAULT_CHUNKS_JSONL: &str =
    "/mnt/d/contextgraph/corpus/python-ast-chunks-v1/python_ast_chunks.jsonl";
const DEFAULT_REGISTRY_JSONL: &str =
    "/mnt/d/contextgraph/corpus/python-frozen-instrument-registry-v1/python_frozen_instrument_records.jsonl";
const DEFAULT_MODELS_CONFIG: &str = "/mnt/d/cabdru/contextgraph/models/mejepa_models_config.toml";
const DEFAULT_CACHE_ROOT: &str = "/mnt/d/contextgraph/corpus/python-forward-cache-v1";
const DEFAULT_FSV_ROOT: &str =
    "/mnt/d/contextgraph/fsv/task-py-data-016-corpus-embedder-explosion-fsv";
const DEFAULT_HOT_ROOT: &str =
    "/home/cabdru/.cache/contextgraph/task-py-data-016-corpus-embedder-explosion";
const CACHE_SCHEMA_VERSION: u16 = 1;
const DEFAULT_MAX_FORWARD_SOURCE_TEXT_BYTES: usize = 32_768;
const DEFAULT_HOT_MODEL_CACHE_MAX_BYTES: u64 = 40 * 1024 * 1024 * 1024;
const HOT_MODEL_CACHE_FREE_SPACE_SLACK_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const DEFAULT_JSONL_SYNC_EVERY_ROWS: usize = 512;
const DEFAULT_TRUE_BATCH_SIZE: usize = 8;
const DEFAULT_GPU_VRAM_GUARD_BYTES: u64 = 32_607 * 1024 * 1024;
const DEFAULT_IO_WORKERS: usize = 2;
const DEFAULT_CPU_WORKERS_FALLBACK: usize = 1;
const DEFAULT_GPU_WORKERS: usize = 1;
const SAMPLE_LIMIT: usize = 20;
const DEFAULT_CACHE_STORE_LAYOUT: &str = "packed";
const DEFAULT_PRETRAINED_MAX_TOKEN_QUADRATIC_ELEMENTS: u64 = 32_000_000;
const DEFAULT_E14_MAX_TOKEN_QUADRATIC_ELEMENTS: u64 =
    DEFAULT_PRETRAINED_MAX_TOKEN_QUADRATIC_ELEMENTS;

#[derive(Debug, Clone, Deserialize)]
struct AstChunk {
    chunk_id: String,
    source_sha256: String,
    file_sha256: String,
    source_text: String,
    absolute_path: String,
    relative_path: String,
    task_instance_id: String,
    workspace_state: String,
    repo: String,
    time: String,
}

#[derive(Debug, Clone, Deserialize)]
struct RegistryRecord {
    instrument_id: String,
    instrument_record_id: String,
    activation_status: String,
    panel_role: String,
    instrument_kind: String,
    output_feature_family: String,
    output_dimension: Option<usize>,
    preprocessing_hash: String,
    stable_hashes: BTreeMap<String, String>,
    target_side_frozen: bool,
    trainable_parameter_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct ForwardCacheKey {
    schema_version: u16,
    artifact_kind: String,
    chunk_id: String,
    source_sha256: String,
    file_sha256: String,
    embedder_id: String,
    instrument_record_id: String,
    preprocessing_hash: String,
    model_hash: String,
    input_sha256: String,
    output_feature_family: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct ChunkRef {
    task_instance_id: String,
    repo: String,
    workspace_state: String,
    absolute_path: String,
    relative_path: String,
    chunk_time: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct ForwardCacheOutput {
    vector: Vec<f32>,
    vector_len: usize,
    vector_sha256: String,
    vector_l2_norm: f64,
    precision_class: String,
    runtime_model_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct ForwardCacheRecord {
    schema_version: u16,
    cache_key_sha256: String,
    key: ForwardCacheKey,
    chunk: ChunkRef,
    instrument_id: String,
    instrument_kind: String,
    output: ForwardCacheOutput,
    source_readback_verified: bool,
}

#[derive(Debug, Clone, Serialize)]
struct CacheAction {
    embedder_id: String,
    cache_key_sha256: String,
    action: String,
    true_batch: bool,
    batch_id: Option<String>,
    batch_size: Option<usize>,
    batch_index: Option<usize>,
    latency_ms: u128,
    record_path: String,
    record_store_layout: String,
    packed_record_path: Option<String>,
    packed_index_path: Option<String>,
    packed_byte_offset: Option<u64>,
    packed_byte_len: Option<u64>,
    record_sha256: String,
    vector_sha256: String,
    vector_len: usize,
    byte_count: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CacheStoreLayout {
    LegacyFiles,
    PackedJsonl,
}

impl CacheStoreLayout {
    fn from_env() -> FsvResult<Self> {
        let raw = std::env::var("CG_MEJEPA_PY_FORWARD_CACHE_STORE_LAYOUT")
            .unwrap_or_else(|_| DEFAULT_CACHE_STORE_LAYOUT.to_string());
        match raw.trim() {
            "packed" | "packed-jsonl" => Ok(Self::PackedJsonl),
            "legacy-files" => Ok(Self::LegacyFiles),
            other => Err(format!(
                "unsupported CG_MEJEPA_PY_FORWARD_CACHE_STORE_LAYOUT={other}; expected packed or legacy-files"
            )
            .into()),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::LegacyFiles => "legacy-files",
            Self::PackedJsonl => "packed-jsonl",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PackedIndexRow {
    schema_version: u16,
    artifact_kind: String,
    cache_key_sha256: String,
    embedder_id: String,
    packed_record_path: String,
    byte_offset: u64,
    byte_len: u64,
    record_sha256: String,
    vector_sha256: String,
    vector_len: usize,
}

#[derive(Debug, Clone)]
struct CacheRecordReadback {
    record: ForwardCacheRecord,
    record_path: String,
    record_store_layout: String,
    packed_record_path: Option<String>,
    packed_index_path: Option<String>,
    packed_byte_offset: Option<u64>,
    packed_byte_len: Option<u64>,
    record_sha256: String,
    byte_count: u64,
}

struct CacheActionContext {
    embedder: EmbedderId,
    key_sha: String,
    action: &'static str,
    true_batch: bool,
    batch_id: Option<String>,
    batch_size: Option<usize>,
    batch_index: Option<usize>,
    latency_ms: u128,
}

fn cache_action_from_readback(
    context: CacheActionContext,
    readback: CacheRecordReadback,
) -> CacheAction {
    CacheAction {
        embedder_id: context.embedder.slug().to_string(),
        cache_key_sha256: context.key_sha,
        action: context.action.to_string(),
        true_batch: context.true_batch,
        batch_id: context.batch_id,
        batch_size: context.batch_size,
        batch_index: context.batch_index,
        latency_ms: context.latency_ms,
        record_path: readback.record_path,
        record_store_layout: readback.record_store_layout,
        packed_record_path: readback.packed_record_path,
        packed_index_path: readback.packed_index_path,
        packed_byte_offset: readback.packed_byte_offset,
        packed_byte_len: readback.packed_byte_len,
        record_sha256: readback.record_sha256,
        vector_sha256: readback.record.output.vector_sha256,
        vector_len: readback.record.output.vector_len,
        byte_count: readback.byte_count,
    }
}

#[derive(Debug, Clone, Serialize)]
struct JsonlWriteState {
    path: String,
    row_count: usize,
    byte_count: u64,
    sha256: String,
    readback_row_count: usize,
}

#[derive(Debug, Clone, Serialize)]
struct StoreState {
    label: String,
    root: String,
    file_count: usize,
    byte_count: u64,
    index_path: String,
    manifest_path: String,
    schema_path: String,
    cache_record_count: usize,
    legacy_record_file_count: usize,
    packed_record_row_count: usize,
    packed_record_file_count: usize,
    cache_record_hash_sample: Vec<String>,
    cache_record_hash_sample_truncated: bool,
    disk: Value,
    hot_root_bytes: u64,
    durable_root_bytes: u64,
}

#[derive(Debug, Default, Clone)]
struct EmbedderCoverageStats {
    first_pass_hits: usize,
    first_pass_writes: usize,
    first_pass_failures: usize,
    second_pass_hits: usize,
    quarantined_pairs: usize,
    first_pass_latency_ms_total: u128,
    second_pass_latency_ms_total: u128,
    vector_lens: BTreeSet<usize>,
}

#[derive(Debug, Clone)]
struct CoverageTracker {
    expected_chunks: usize,
    by_embedder: BTreeMap<String, EmbedderCoverageStats>,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> FsvResult<()> {
    let paths = Paths::from_env();
    let max_chunks = env_usize("CG_MEJEPA_PY_FORWARD_CACHE_MAX_CHUNKS", 1)?;
    let chunk_offset = env_usize("CG_MEJEPA_PY_FORWARD_CACHE_CHUNK_OFFSET", 0)?;
    let max_source_bytes = env_usize(
        "CG_MEJEPA_PY_FORWARD_MAX_SOURCE_BYTES",
        DEFAULT_MAX_FORWARD_SOURCE_TEXT_BYTES,
    )?;
    let hot_model_cache_max_bytes = env_u64(
        "CG_MEJEPA_PY_FORWARD_HOT_MODEL_CACHE_MAX_BYTES",
        DEFAULT_HOT_MODEL_CACHE_MAX_BYTES,
    )?;
    let true_batch_size = env_usize(
        "CG_MEJEPA_PY_FORWARD_TRUE_BATCH_SIZE",
        DEFAULT_TRUE_BATCH_SIZE,
    )?;
    let gpu_vram_guard_bytes = env_u64(
        "CG_MEJEPA_PY_FORWARD_GPU_VRAM_GUARD_BYTES",
        DEFAULT_GPU_VRAM_GUARD_BYTES,
    )?;
    let io_workers = env_usize("CG_MEJEPA_PY_FORWARD_IO_WORKERS", DEFAULT_IO_WORKERS)?;
    let cpu_workers = env_usize("CG_MEJEPA_PY_FORWARD_CPU_WORKERS", default_cpu_workers())?;
    let gpu_workers = env_usize("CG_MEJEPA_PY_FORWARD_GPU_WORKERS", DEFAULT_GPU_WORKERS)?;
    let jsonl_sync_every_rows = env_usize(
        "CG_MEJEPA_PY_FORWARD_JSONL_SYNC_EVERY_ROWS",
        DEFAULT_JSONL_SYNC_EVERY_ROWS,
    )?;
    let cache_store_layout = CacheStoreLayout::from_env()?;
    let only_embedder = env_embedder_optional("CG_MEJEPA_PY_FORWARD_ONLY_EMBEDDER")?;
    require("max chunk count is non-zero", max_chunks > 0)?;
    require("max source byte guard is non-zero", max_source_bytes > 0)?;
    require(
        "hot model cache byte budget is non-zero",
        hot_model_cache_max_bytes > 0,
    )?;
    require("true batch size is non-zero", true_batch_size > 0)?;
    require("GPU VRAM guard is non-zero", gpu_vram_guard_bytes > 0)?;
    require("D-drive I/O worker count is non-zero", io_workers > 0)?;
    require("CPU worker count is non-zero", cpu_workers > 0)?;
    require("GPU worker count is non-zero", gpu_workers > 0)?;
    require(
        "GPU worker count remains single-writer for one local RTX 5090",
        gpu_workers == 1,
    )?;

    fs::create_dir_all(&paths.cache_root)?;
    fs::create_dir_all(&paths.fsv_root)?;
    fs::create_dir_all(&paths.hot_root)?;
    fs::set_permissions(&paths.hot_root, fs::Permissions::from_mode(0o700))?;

    let run_started_at = utc_now()?;
    write_json_checked(
        &paths.manifest_path(),
        &json!({
            "schema_version": CACHE_SCHEMA_VERSION,
            "task_id": "TASK-PY-DATA-016",
            "status": "running",
            "passes": false,
            "started_at_utc": run_started_at,
            "source_of_truth": {
                "cache_root": paths.cache_root.display().to_string(),
                "index_jsonl": paths.index_path().display().to_string(),
                "manifest": paths.manifest_path().display().to_string(),
                "schema": paths.schema_path().display().to_string(),
                "quarantine_jsonl": paths.quarantine_path().display().to_string(),
                "batch_telemetry_jsonl": paths.batch_telemetry_path().display().to_string(),
                "packed_manifest": paths.packed_manifest_path().display().to_string(),
                "packed_record_root": paths.packed_root().display().to_string(),
            },
            "inputs": {
                "chunks_jsonl": paths.chunks_jsonl.display().to_string(),
                "chunk_offset": chunk_offset,
                "max_chunks": max_chunks,
                "true_batch_size": true_batch_size,
                "cache_store_layout": cache_store_layout.as_str(),
                "only_embedder": only_embedder.map(|embedder| embedder.slug().to_string()),
            },
            "reason": "run started; final manifest is written only after cache/index/quarantine/FSV readback succeeds",
        }),
    )?;
    write_jsonl_checked(&paths.index_path(), &[])?;
    write_jsonl_checked(&paths.quarantine_path(), &[])?;
    write_jsonl_checked(&paths.batch_telemetry_path(), &[])?;

    let before = store_state("before", &paths)?;
    let chunks = load_chunks(&paths.chunks_jsonl, chunk_offset, max_chunks)?;
    let registry = load_active_embedder_records(&paths.registry_jsonl, only_embedder)?;
    verify_registry_matches_runtime(&registry, only_embedder)?;
    let batch_controls = build_true_batch_controls(&registry, true_batch_size, max_source_bytes)?;
    let config = ModelsConfig::load(&paths.models_config)?;
    let mut cache_store = ForwardCacheStore::open(&paths, cache_store_layout)?;

    let mut coverage_tracker = CoverageTracker::new(&registry, chunks.len())?;
    let mut first_pass_count = 0usize;
    let mut first_pass_sample = Vec::new();
    let mut second_pass_count = 0usize;
    let mut second_pass_sample = Vec::new();
    let mut quarantine_sample = Vec::new();
    let mut quarantined_pairs = BTreeSet::new();
    let mut index_sink = JsonlSink::create(&paths.index_path(), jsonl_sync_every_rows)?;
    let mut quarantine_sink = JsonlSink::create(&paths.quarantine_path(), jsonl_sync_every_rows)?;
    let mut batch_telemetry_sink =
        JsonlSink::create(&paths.batch_telemetry_path(), jsonl_sync_every_rows)?;
    for record in &registry {
        if is_true_batch_forward_embedder(record.embedder_id()?) {
            let embedder = record.embedder_id()?;
            let controls = batch_controls
                .get(&embedder)
                .ok_or_else(|| format!("missing batch controls for {}", embedder.slug()))?;
            first_pass_true_batch_record(
                &paths,
                &config,
                &chunks,
                record,
                max_source_bytes,
                hot_model_cache_max_bytes,
                controls,
                gpu_vram_guard_bytes,
                &mut cache_store,
                &mut index_sink,
                &mut quarantine_sink,
                &mut batch_telemetry_sink,
                &mut coverage_tracker,
                &mut first_pass_count,
                &mut first_pass_sample,
                &mut quarantine_sample,
                &mut quarantined_pairs,
            )
            .await?;
        } else {
            let mut loaded = None;
            let embedder = record.embedder_id()?;
            let serial_started = Instant::now();
            let mut serial_row_summaries = Vec::new();
            let mut serial_quarantine_count = 0usize;
            let mut serial_source_byte_total = 0usize;
            let mut serial_source_byte_max = 0usize;
            for chunk in &chunks {
                if let Some((code, reason)) = chunk_non_forwardable_reason(chunk, max_source_bytes)
                {
                    let row = quarantine_row(chunk, record, code, reason, max_source_bytes)?;
                    quarantine_sink.write_row(&row)?;
                    coverage_tracker.record_quarantine(record)?;
                    record_quarantine_pair(&mut quarantined_pairs, chunk, record)?;
                    push_sample(&mut quarantine_sample, row);
                    serial_quarantine_count += 1;
                    continue;
                }
                let action = cache_or_forward(
                    &paths,
                    &config,
                    chunk,
                    record,
                    &mut loaded,
                    max_source_bytes,
                    hot_model_cache_max_bytes,
                    &mut cache_store,
                )
                .await?;
                let row = index_row(chunk, record, &action);
                index_sink.write_row(&row)?;
                coverage_tracker.record_first(record, &action)?;
                first_pass_count += 1;
                serial_source_byte_total = serial_source_byte_total
                    .checked_add(chunk.source_text.len())
                    .ok_or("serial algorithmic source byte total overflow")?;
                serial_source_byte_max = serial_source_byte_max.max(chunk.source_text.len());
                if serial_row_summaries.len() < SAMPLE_LIMIT {
                    serial_row_summaries.push(json!({
                        "chunk_id": chunk.chunk_id,
                        "cache_key_sha256": action.cache_key_sha256,
                        "action": action.action,
                        "record_path": action.record_path,
                        "record_store_layout": action.record_store_layout,
                        "vector_len": action.vector_len,
                        "vector_sha256": action.vector_sha256,
                        "latency_ms": action.latency_ms,
                    }));
                }
                push_sample(&mut first_pass_sample, action);
            }
            if matches!(embedder, EmbedderId::E2 | EmbedderId::E3 | EmbedderId::E4) {
                let serial_processed_count = chunks.len().saturating_sub(serial_quarantine_count);
                batch_telemetry_sink.write_row(&json!({
                    "schema_version": CACHE_SCHEMA_VERSION,
                    "task_id": "TASK-PY-DATA-039",
                    "artifact_kind": "python_forward_algorithmic_serial_telemetry",
                    "timestamp_utc": utc_now()?,
                    "embedder_id": embedder.slug(),
                    "instrument_id": record.instrument_id,
                    "instrument_record_id": record.instrument_record_id,
                    "true_batch": false,
                    "cuda_applicable": false,
                    "reason_code": "MEJEPA_ALGORITHMIC_TEMPORAL_EXACT_PATH_NOT_GPU_BATCHED",
                    "reason": "E2/E3/E4 use exact timestamp/session hash and f64 sin/cos paths; they remain fail-closed for CUDA true-batch until an exact-output kernel proves vector-hash equivalence",
                    "row_count": serial_processed_count,
                    "rows_sample_count": serial_row_summaries.len(),
                    "processed_non_quarantine_count": serial_processed_count,
                    "quarantine_count": serial_quarantine_count,
                    "source_stats": {
                        "row_count": serial_processed_count,
                        "source_byte_total": serial_source_byte_total,
                        "source_byte_max": serial_source_byte_max,
                    },
                    "output_dimension": embedder.dimension(),
                    "rows_sample": serial_row_summaries,
                    "latency_ms": serial_started.elapsed().as_millis(),
                }))?;
            }
        }
    }
    let index_write_state = index_sink.finish()?;
    let quarantine_write_state = quarantine_sink.finish()?;
    let batch_telemetry_write_state = batch_telemetry_sink.finish()?;
    require(
        "batch telemetry exists when the runner performs first-pass writes",
        batch_telemetry_write_state.row_count > 0 || coverage_tracker.first_pass_writes() == 0,
    )?;

    for chunk in &chunks {
        for record in &registry {
            if chunk_non_forwardable_reason(chunk, max_source_bytes).is_some() {
                continue;
            }
            if quarantined_pairs.contains(&quarantine_pair_key(chunk, record)?) {
                continue;
            }
            let key = build_key(chunk, record, max_source_bytes)?;
            let embedder = record.embedder_id()?;
            let readback = cache_store.read(&paths, embedder.slug(), &key)?;
            validate_record(&readback.record, record)?;
            let action = cache_action_from_readback(
                CacheActionContext {
                    embedder,
                    key_sha: key_hash(&key)?,
                    action: "hit",
                    true_batch: false,
                    batch_id: None,
                    batch_size: None,
                    batch_index: None,
                    latency_ms: 0,
                },
                readback,
            );
            coverage_tracker.record_second(record, &action)?;
            second_pass_count += 1;
            push_sample(&mut second_pass_sample, action);
        }
    }

    let schema = schema_json();
    write_json_checked(&paths.schema_path(), &schema)?;
    let manifest_state = store_state("manifest_before_final_write", &paths)?;
    let coverage = coverage_tracker.summary(&registry)?;
    let manifest = json!({
        "schema_version": CACHE_SCHEMA_VERSION,
        "task_id": "TASK-PY-DATA-016",
        "status": "passed",
        "started_at_utc": run_started_at,
        "created_at_utc": utc_now()?,
        "source_of_truth": {
            "cache_root": paths.cache_root.display().to_string(),
            "index_jsonl": paths.index_path().display().to_string(),
                "manifest": paths.manifest_path().display().to_string(),
                "schema": paths.schema_path().display().to_string(),
                "quarantine_jsonl": paths.quarantine_path().display().to_string(),
                "batch_telemetry_jsonl": paths.batch_telemetry_path().display().to_string(),
                "packed_manifest": paths.packed_manifest_path().display().to_string(),
                "packed_record_root": paths.packed_root().display().to_string(),
            },
            "inputs": {
                "chunks_jsonl": paths.chunks_jsonl.display().to_string(),
                "registry_jsonl": paths.registry_jsonl.display().to_string(),
                "models_config": paths.models_config.display().to_string(),
            "chunk_offset": chunk_offset,
            "max_chunks": max_chunks,
                "only_embedder": only_embedder.map(|embedder| embedder.slug().to_string()),
                "max_forward_source_text_bytes": max_source_bytes,
                "cache_store_layout": cache_store.layout().as_str(),
                "hot_model_cache_root": paths.hot_model_cache_root().display().to_string(),
                "hot_model_cache_max_bytes": hot_model_cache_max_bytes,
                "true_batch_size": true_batch_size,
                "per_embedder_batch_controls": batch_controls
                    .values()
                    .cloned()
                    .collect::<Vec<_>>(),
                "gpu_vram_guard_bytes": gpu_vram_guard_bytes,
                "worker_tiers": {
                    "d_drive_io_workers": io_workers,
                    "cpu_static_tokenization_workers": cpu_workers,
                    "gpu_batch_workers": gpu_workers,
                },
            },
            "summary": {
                "chunk_count": chunks.len(),
                "active_embedder_count": registry.len(),
                "cache_record_count": first_pass_count,
            "first_pass_writes": coverage_tracker.first_pass_writes(),
            "first_pass_hits": coverage_tracker.first_pass_hits(),
                "second_pass_hits": coverage_tracker.second_pass_hits(),
                "quarantine_row_count": coverage_tracker.quarantined_pairs(),
                "global_cache_record_count_at_manifest_write": manifest_state.cache_record_count,
                "batch_telemetry_row_count": batch_telemetry_write_state.row_count,
            },
        "per_embedder_coverage": coverage,
        "jsonl_write_state": {
            "index": index_write_state,
            "quarantine": quarantine_write_state,
            "batch_telemetry": batch_telemetry_write_state,
            "sync_every_rows": jsonl_sync_every_rows,
        },
        "packed_store": cache_store.write_manifest(&paths)?,
        "quarantine_policy": {
            "MEJEPA_FORWARD_EMPTY_CHUNK": "accepted Python file has empty or whitespace-only source_text; no text embedding can be computed, so every embedder pair is explicitly quarantined",
            "MEJEPA_FORWARD_INPUT_TOO_LARGE": "accepted Python AST chunk exceeds max_forward_source_text_bytes; chunk must be split or explicitly accepted as missing-output quarantine before full corpus success",
            "MEJEPA_FORWARD_TOKEN_TOO_LONG": "model tokenizer rejected this chunk under the active pretrained embedder token limit; the row is isolated from the true batch and explicitly quarantined with no vector output",
            "MEJEPA_FORWARD_GPU_OOM": "GPU memory exhaustion remained after true-batch binary isolation reached a single row; no CPU or deterministic vector fallback is permitted, so the missing embedder pair is explicitly quarantined with full error evidence",
        },
        "quarantine_row_count": coverage_tracker.quarantined_pairs(),
        "index_sha256": sha256_file(&paths.index_path())?,
        "schema_sha256": sha256_file(&paths.schema_path())?,
        "quarantine_sha256": sha256_file(&paths.quarantine_path())?,
        "passes": true,
    });
    write_json_checked(&paths.manifest_path(), &manifest)?;

    let mut edge_cases = vec![
        edge_empty_chunk(&paths, &registry[0])?,
        edge_embedder_mismatch(&paths).await?,
        edge_unsupported_true_batch(&paths).await?,
        edge_invalid_embedder_batch_config(&paths)?,
        edge_batch_planner_source_budget(&paths)?,
        edge_token_quadratic_batch_planner(&paths)?,
        edge_pretrained_token_quadratic_defaults(&paths)?,
    ];
    if registry.iter().any(|record| {
        record.embedder_id().map_or(false, |embedder| {
            SUPPORTED_PRETRAINED_FORWARD_EMBEDDERS.contains(&embedder)
        })
    }) {
        edge_cases.push(edge_true_batch_vram_guard(
            &paths,
            &registry,
            max_source_bytes,
        )?);
    } else {
        edge_cases.push(edge_not_applicable(
            &paths,
            "true_batch_vram_guard_not_applicable_for_isolated_algorithmic_embedder",
            "single-embedder process isolation selected only an algorithmic wrapper, so no pretrained VRAM guard is evaluated in this subrun",
        )?);
    }
    edge_cases.extend([
        edge_true_batch_gpu_oom_classifier(&paths)?,
        edge_hot_stage_promotion_mismatch(&paths)?,
        edge_cache_corruption(&paths)?,
        edge_packed_duplicate_key(&paths)?,
        edge_packed_corrupt_row(&paths)?,
        edge_packed_missing_payload(&paths)?,
        edge_packed_truncated_payload(&paths)?,
    ]);
    if registry
        .iter()
        .any(|record| matches!(record.embedder_id(), Ok(EmbedderId::E9)))
    {
        edge_cases.push(edge_maximum_size_chunk(&paths, &registry, max_source_bytes).await?);
    } else {
        edge_cases.push(edge_not_applicable(
            &paths,
            "maximum_size_e9_edge_not_applicable_for_isolated_embedder",
            "single-embedder process isolation selected a registry slice that does not include E9; the final all-embedder readback pass still runs the E9 maximum-size edge",
        )?);
    }
    edge_cases.push(edge_dimension_mismatch(
        &paths,
        &chunks[0],
        &registry[0],
        max_source_bytes,
    )?);
    let after = store_state("after", &paths)?;
    let fsv = json!({
        "source_of_truth": {
            "cache_root": paths.cache_root.display().to_string(),
            "index_jsonl": paths.index_path().display().to_string(),
            "manifest": paths.manifest_path().display().to_string(),
            "schema": paths.schema_path().display().to_string(),
            "quarantine_jsonl": paths.quarantine_path().display().to_string(),
            "batch_telemetry_jsonl": paths.batch_telemetry_path().display().to_string(),
            "packed_manifest": paths.packed_manifest_path().display().to_string(),
            "packed_record_root": paths.packed_root().display().to_string(),
        },
        "happy_path": {
            "before_state": before,
            "trigger": "read Python AST chunk records and frozen active embedder registry; forward or hit cache for each active embedder; then re-read same cache keys",
            "first_pass_count": first_pass_count,
            "first_pass_sample": first_pass_sample,
            "second_pass_count": second_pass_count,
            "second_pass_sample": second_pass_sample,
            "quarantine_row_count": coverage_tracker.quarantined_pairs(),
            "quarantine_sample": quarantine_sample,
            "per_embedder_coverage": coverage,
            "after_state": after,
            "expected": {
                "second_pass_hits": first_pass_count,
                "cache_records_match_index_rows": true,
                "full_vectors_persisted": true,
                "missing_outputs_have_quarantine_rows": true
            },
            "passes": true,
        },
        "edge_cases": edge_cases,
        "capacity_policy": {
            "hot_root": paths.hot_root.display().to_string(),
            "hot_model_cache_root": paths.hot_model_cache_root().display().to_string(),
            "record_stage_root": paths.record_stage_root().display().to_string(),
            "durable_root": paths.cache_root.display().to_string(),
            "hot_root_allowed": true,
            "durable_root_on_d_drive": paths.cache_root.starts_with("/mnt/d"),
            "chunk_offset": chunk_offset,
            "max_chunks": max_chunks,
            "only_embedder": only_embedder.map(|embedder| embedder.slug().to_string()),
            "max_forward_source_text_bytes": max_source_bytes,
            "hot_model_cache_max_bytes": hot_model_cache_max_bytes,
            "cache_store_layout": cache_store.layout().as_str(),
            "true_batch_size": true_batch_size,
            "per_embedder_batch_controls": batch_controls
                .values()
                .cloned()
                .collect::<Vec<_>>(),
            "gpu_vram_guard_bytes": gpu_vram_guard_bytes,
            "worker_tiers": {
                "d_drive_io_workers": io_workers,
                "cpu_static_tokenization_workers": cpu_workers,
                "gpu_batch_workers": gpu_workers,
            },
        },
        "passes": true,
    });
    write_json_checked(&paths.fsv_path(), &fsv)?;
    let readback: Value = serde_json::from_slice(&fs::read(paths.fsv_path())?)?;
    require(
        "FSV readback passes",
        readback["passes"] == Value::Bool(true),
    )?;
    println!("{}", serde_json::to_string_pretty(&fsv)?);
    Ok(())
}

#[derive(Debug)]
struct Paths {
    chunks_jsonl: PathBuf,
    registry_jsonl: PathBuf,
    models_config: PathBuf,
    cache_root: PathBuf,
    fsv_root: PathBuf,
    hot_root: PathBuf,
}

impl Paths {
    fn from_env() -> Self {
        Self {
            chunks_jsonl: env_path("CG_MEJEPA_PY_CHUNKS_JSONL", DEFAULT_CHUNKS_JSONL),
            registry_jsonl: env_path(
                "CG_MEJEPA_PY_INSTRUMENT_REGISTRY_JSONL",
                DEFAULT_REGISTRY_JSONL,
            ),
            models_config: env_path("CONTEXTGRAPH_MEJEPA_MODELS_CONFIG", DEFAULT_MODELS_CONFIG),
            cache_root: env_path("CG_MEJEPA_PY_FORWARD_CACHE_ROOT", DEFAULT_CACHE_ROOT),
            fsv_root: env_path("CG_MEJEPA_PY_FORWARD_CACHE_FSV_ROOT", DEFAULT_FSV_ROOT),
            hot_root: env_path("CG_MEJEPA_PY_FORWARD_CACHE_HOT_ROOT", DEFAULT_HOT_ROOT),
        }
    }

    fn index_path(&self) -> PathBuf {
        self.cache_root.join("python_forward_cache_index.jsonl")
    }

    fn manifest_path(&self) -> PathBuf {
        self.cache_root.join("python_forward_cache_manifest.json")
    }

    fn schema_path(&self) -> PathBuf {
        self.cache_root.join("python_forward_cache_schema.json")
    }

    fn quarantine_path(&self) -> PathBuf {
        self.cache_root
            .join("python_forward_cache_quarantine.jsonl")
    }

    fn batch_telemetry_path(&self) -> PathBuf {
        self.cache_root
            .join("python_forward_cache_batch_telemetry.jsonl")
    }

    fn packed_root(&self) -> PathBuf {
        self.cache_root.join("packed_records")
    }

    fn packed_record_path(&self, embedder: &str) -> PathBuf {
        self.packed_root().join(format!("{embedder}.jsonl"))
    }

    fn packed_index_path(&self, embedder: &str) -> PathBuf {
        self.packed_root().join(format!("{embedder}.index.jsonl"))
    }

    fn packed_manifest_path(&self) -> PathBuf {
        self.cache_root
            .join("python_forward_cache_packed_manifest.json")
    }

    fn progress_path(&self) -> PathBuf {
        self.hot_root.join("python_forward_cache_progress.json")
    }

    fn hot_model_cache_root(&self) -> PathBuf {
        self.hot_root.join("model-artifact-cache")
    }

    fn record_stage_root(&self) -> PathBuf {
        self.hot_root.join("record-promotion-stage")
    }

    fn fsv_path(&self) -> PathBuf {
        self.fsv_root.join("python_forward_cache_fsv.json")
    }
}

impl RegistryRecord {
    fn embedder_id(&self) -> FsvResult<EmbedderId> {
        for embedder in EmbedderId::content() {
            if self
                .instrument_id
                .contains(&format!("_{}_", embedder.slug()))
            {
                return Ok(embedder);
            }
        }
        Err(format!("could not derive embedder id from {}", self.instrument_id).into())
    }

    fn model_hash(&self) -> FsvResult<String> {
        let value = self
            .stable_hashes
            .get("weight_or_algorithm")
            .ok_or_else(|| format!("{} missing weight_or_algorithm hash", self.instrument_id))?;
        require(
            "model hash is sha256-prefixed",
            value.starts_with("sha256:") && value.len() == 71,
        )?;
        Ok(value.clone())
    }
}

enum LoadedForward {
    Algorithmic(AlgorithmicEmbedderForward),
    Pretrained(PretrainedEmbedderForward),
}

impl LoadedForward {
    async fn load(
        paths: &Paths,
        config: &ModelsConfig,
        embedder: EmbedderId,
        chunk: &AstChunk,
        record: &RegistryRecord,
        hot_model_cache_max_bytes: u64,
    ) -> FsvResult<Self> {
        if matches!(
            embedder,
            EmbedderId::E2 | EmbedderId::E3 | EmbedderId::E4 | EmbedderId::E9
        ) {
            return Ok(Self::Algorithmic(AlgorithmicEmbedderForward::load(
                embedder,
            )?));
        }
        let registration = config.registration(embedder)?;
        let registration = materialize_hot_model_registration(
            paths,
            chunk,
            record,
            registration,
            hot_model_cache_max_bytes,
        )?;
        write_progress(
            paths,
            chunk,
            record,
            "pretrained_runtime_load_start",
            json!({
                "staged_model_root": registration.path.clone(),
                "manifest_sha256": registration.manifest_sha256.clone(),
                "weight_files": registration.weight_files.clone(),
            }),
        )?;
        let load_started = Instant::now();
        let forward = PretrainedEmbedderForward::load(&registration).await?;
        write_progress(
            paths,
            chunk,
            record,
            "pretrained_runtime_load_complete",
            json!({
                "staged_model_root": registration.path.clone(),
                "manifest_sha256": registration.manifest_sha256.clone(),
                "latency_ms": load_started.elapsed().as_millis(),
            }),
        )?;
        Ok(Self::Pretrained(forward))
    }

    fn supports_true_batch(&self) -> bool {
        match self {
            Self::Algorithmic(forward) => forward.supports_true_batch(),
            Self::Pretrained(forward) => forward.supports_true_batch(),
        }
    }

    async fn forward(&self, input: &EmbedderInput) -> FsvResult<EmbedderOutput> {
        let output = match self {
            Self::Algorithmic(forward) => forward.forward(input).await?,
            Self::Pretrained(forward) => forward.forward(input).await?,
        };
        output.validate()?;
        require(
            "forward output vector is non-empty",
            !output.vector.is_empty() && output.vector_l2_norm() > 0.0,
        )?;
        Ok(output)
    }

    async fn forward_true_batch(&self, inputs: &[EmbedderInput]) -> FsvResult<Vec<EmbedderOutput>> {
        let outputs = match self {
            Self::Algorithmic(forward) => forward.forward_true_batch(inputs).await?,
            Self::Pretrained(forward) => forward.forward_true_batch(inputs).await?,
        };
        require(
            "true-batch output count matches input count",
            outputs.len() == inputs.len(),
        )?;
        for output in &outputs {
            output.validate()?;
            require(
                "true-batch output vector is non-empty",
                !output.vector.is_empty() && output.vector_l2_norm() > 0.0,
            )?;
        }
        Ok(outputs)
    }
}

fn materialize_hot_model_registration(
    paths: &Paths,
    chunk: &AstChunk,
    record: &RegistryRecord,
    registration: &EmbedderRegistration,
    hot_model_cache_max_bytes: u64,
) -> FsvResult<EmbedderRegistration> {
    registration.validate()?;
    let source_root = Path::new(&registration.path);
    require("pretrained model source root exists", source_root.is_dir())?;
    let hot_root = paths.hot_model_cache_root();
    require(
        "hot model cache root is WSL ext4, not a Windows mount",
        !hot_root.starts_with("/mnt/c") && !hot_root.starts_with("/mnt/d"),
    )?;
    fs::create_dir_all(&hot_root)?;
    fs::set_permissions(&hot_root, fs::Permissions::from_mode(0o700))?;

    let staged_root = hot_root.join(&registration.manifest_sha256);
    fs::create_dir_all(&staged_root)?;
    fs::set_permissions(&staged_root, fs::Permissions::from_mode(0o700))?;

    let mut total_pinned_bytes = 0u64;
    let mut planned_copy_bytes = 0u64;
    let mut plan_files = Vec::new();
    let mut weight_files = registration.weight_files.clone();
    weight_files.sort();
    for relative in &weight_files {
        let source = source_root.join(relative);
        let target = staged_root.join(relative);
        let metadata = fs::metadata(&source)?;
        require(
            "pinned model artifact is a regular file",
            metadata.is_file(),
        )?;
        let source_bytes = metadata.len();
        total_pinned_bytes = total_pinned_bytes.saturating_add(source_bytes);
        let existing_bytes = fs::metadata(&target).ok().map(|meta| meta.len());
        let needs_copy = existing_bytes != Some(source_bytes);
        if needs_copy {
            planned_copy_bytes = planned_copy_bytes.saturating_add(source_bytes);
        }
        plan_files.push(json!({
            "relative_path": relative,
            "source_path": source.display().to_string(),
            "target_path": target.display().to_string(),
            "source_bytes": source_bytes,
            "target_existing_bytes": existing_bytes,
            "needs_copy": needs_copy,
        }));
    }

    let hot_available_before = available_bytes(&hot_root)?;
    require(
        "pinned model artifact set fits hot model cache budget",
        total_pinned_bytes <= hot_model_cache_max_bytes,
    )?;
    require(
        "planned hot model copy fits hot model cache budget",
        planned_copy_bytes <= hot_model_cache_max_bytes,
    )?;
    require(
        "hot model cache has enough free bytes for planned copy plus slack",
        planned_copy_bytes.saturating_add(HOT_MODEL_CACHE_FREE_SPACE_SLACK_BYTES)
            <= hot_available_before,
    )?;
    write_progress(
        paths,
        chunk,
        record,
        "hot_model_stage_plan",
        json!({
            "source_root": source_root.display().to_string(),
            "staged_root": staged_root.display().to_string(),
            "hot_model_cache_root": hot_root.display().to_string(),
            "hot_available_before_bytes": hot_available_before,
            "hot_model_cache_max_bytes": hot_model_cache_max_bytes,
            "total_pinned_bytes": total_pinned_bytes,
            "planned_copy_bytes": planned_copy_bytes,
            "files": plan_files,
        }),
    )?;

    let mut materialized_files = Vec::new();
    for relative in &weight_files {
        let source = source_root.join(relative);
        let target = staged_root.join(relative);
        let source_bytes = fs::metadata(&source)?.len();
        if fs::metadata(&target).ok().map(|meta| meta.len()) != Some(source_bytes) {
            write_progress(
                paths,
                chunk,
                record,
                "hot_model_file_copy_start",
                json!({
                    "relative_path": relative,
                    "source_path": source.display().to_string(),
                    "target_path": target.display().to_string(),
                    "source_bytes": source_bytes,
                }),
            )?;
            copy_file_atomic_checked(&source, &target, source_bytes)?;
            materialized_files.push(json!({
                "relative_path": relative,
                "action": "copied",
                "source_bytes": source_bytes,
                "target_bytes": fs::metadata(&target)?.len(),
            }));
            write_progress(
                paths,
                chunk,
                record,
                "hot_model_file_copy_complete",
                json!({
                    "relative_path": relative,
                    "source_path": source.display().to_string(),
                    "target_path": target.display().to_string(),
                    "source_bytes": source_bytes,
                    "target_bytes": fs::metadata(&target)?.len(),
                }),
            )?;
        } else {
            materialized_files.push(json!({
                "relative_path": relative,
                "action": "existing_size_match",
                "source_bytes": source_bytes,
                "target_bytes": fs::metadata(&target)?.len(),
            }));
        }
    }

    let digest_started = Instant::now();
    let (manifest_sha256, file_digests) = digest_manifest_for_embedder(
        registration.embedder,
        &staged_root,
        &registration.weight_files,
    )?;
    require(
        "hot-staged model manifest digest matches registration",
        manifest_sha256 == registration.manifest_sha256,
    )?;
    let hot_available_after = available_bytes(&hot_root)?;
    write_progress(
        paths,
        chunk,
        record,
        "hot_model_stage_verified",
        json!({
            "source_root": source_root.display().to_string(),
            "staged_root": staged_root.display().to_string(),
            "hot_model_cache_root": hot_root.display().to_string(),
            "hot_available_before_bytes": hot_available_before,
            "hot_available_after_bytes": hot_available_after,
            "hot_model_cache_max_bytes": hot_model_cache_max_bytes,
            "total_pinned_bytes": total_pinned_bytes,
            "planned_copy_bytes": planned_copy_bytes,
            "manifest_sha256": manifest_sha256,
            "digest_latency_ms": digest_started.elapsed().as_millis(),
            "materialized_files": materialized_files,
            "file_digests": file_digests,
        }),
    )?;

    let mut staged = registration.clone();
    staged.path = staged_root.display().to_string();
    Ok(staged)
}

fn copy_file_atomic_checked(source: &Path, target: &Path, expected_bytes: u64) -> FsvResult<()> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = target.with_extension(format!(
        "{}.tmp-{}-{}",
        target
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("model"),
        std::process::id(),
        unix_ms()?
    ));
    let copied = fs::copy(source, &tmp)?;
    require(
        "model artifact copy byte count matches source metadata",
        copied == expected_bytes,
    )?;
    require(
        "temporary copied model artifact size matches source metadata",
        fs::metadata(&tmp)?.len() == expected_bytes,
    )?;
    File::open(&tmp)?.sync_all()?;
    fs::rename(&tmp, target)?;
    if let Some(parent) = target.parent() {
        File::open(parent)?.sync_all()?;
    }
    require(
        "hot-staged model artifact size readback matches source metadata",
        fs::metadata(target)?.len() == expected_bytes,
    )?;
    Ok(())
}

#[derive(Debug, Clone)]
struct PendingForward {
    chunk_index: usize,
    key: ForwardCacheKey,
    key_sha: String,
    path: PathBuf,
    input: EmbedderInput,
    source_byte_len: usize,
}

#[derive(Debug, Clone, Serialize)]
struct EmbedderBatchControls {
    embedder_id: String,
    max_rows: usize,
    hard_max_rows: Option<usize>,
    max_padded_source_bytes: u64,
    max_token_quadratic_elements: u64,
    row_env_var: String,
    padded_source_env_var: String,
    token_quadratic_env_var: String,
    planner: String,
}

#[derive(Debug, Clone, Serialize)]
struct BatchSourceStats {
    row_count: usize,
    source_byte_min: usize,
    source_byte_max: usize,
    source_byte_sum: usize,
    padded_source_bytes: u64,
    token_estimate_min: usize,
    token_estimate_max: usize,
    token_estimate_sum: usize,
    padded_token_quadratic_elements: u64,
}

#[derive(Debug, Clone, Serialize)]
struct RunnerBatchVramPlan {
    embedder_id: String,
    model_id: String,
    batch_size: usize,
    source_byte_max: usize,
    model_vram_bytes: u64,
    input_budget_bytes: u64,
    output_budget_bytes: u64,
    attention_budget_bytes: u64,
    padded_token_quadratic_elements: u64,
    required_bytes: u64,
    guard_bytes: u64,
}

fn build_true_batch_controls(
    registry: &[RegistryRecord],
    global_true_batch_size: usize,
    max_source_bytes: usize,
) -> FsvResult<BTreeMap<EmbedderId, EmbedderBatchControls>> {
    require(
        "global true-batch row size is non-zero",
        global_true_batch_size > 0,
    )?;
    require("max source bytes is non-zero", max_source_bytes > 0)?;
    let mut controls = BTreeMap::new();
    for record in registry {
        let embedder = record.embedder_id()?;
        if !is_true_batch_forward_embedder(embedder) {
            continue;
        }
        let control = embedder_batch_controls(embedder, global_true_batch_size, max_source_bytes)?;
        require(
            "one batch control row per active true-batch embedder",
            controls.insert(embedder, control).is_none(),
        )?;
    }
    Ok(controls)
}

fn is_true_batch_forward_embedder(embedder: EmbedderId) -> bool {
    SUPPORTED_PRETRAINED_FORWARD_EMBEDDERS.contains(&embedder) || embedder == EmbedderId::E9
}

fn embedder_batch_controls(
    embedder: EmbedderId,
    global_true_batch_size: usize,
    max_source_bytes: usize,
) -> FsvResult<EmbedderBatchControls> {
    let slug_upper = embedder.slug().to_ascii_uppercase();
    let row_env_var = format!("CG_MEJEPA_PY_FORWARD_TRUE_BATCH_SIZE_{slug_upper}");
    let padded_source_env_var =
        format!("CG_MEJEPA_PY_FORWARD_TRUE_BATCH_MAX_PADDED_SOURCE_BYTES_{slug_upper}");
    let token_quadratic_env_var =
        format!("CG_MEJEPA_PY_FORWARD_TRUE_BATCH_MAX_TOKEN_QUADRATIC_{slug_upper}");
    let global_padded_env_var = "CG_MEJEPA_PY_FORWARD_TRUE_BATCH_MAX_PADDED_SOURCE_BYTES";
    let global_token_quadratic_env_var = "CG_MEJEPA_PY_FORWARD_TRUE_BATCH_MAX_TOKEN_QUADRATIC";
    let max_rows = env_usize_optional(&row_env_var)?.unwrap_or(global_true_batch_size);
    let default_padded_source_bytes = (global_true_batch_size as u64)
        .checked_mul(max_source_bytes as u64)
        .ok_or("default padded source-byte budget overflow")?;
    let max_padded_source_bytes = env_u64_optional(&padded_source_env_var)?
        .or(env_u64_optional(global_padded_env_var)?)
        .unwrap_or(default_padded_source_bytes);
    let max_token_quadratic_elements = env_u64_optional(&token_quadratic_env_var)?
        .or(env_u64_optional(global_token_quadratic_env_var)?)
        .unwrap_or(default_token_quadratic_elements(
            embedder,
            global_true_batch_size,
            max_source_bytes,
        )?);
    let controls = EmbedderBatchControls {
        embedder_id: embedder.slug().to_string(),
        max_rows,
        hard_max_rows: embedder_hard_max_rows(embedder),
        max_padded_source_bytes,
        max_token_quadratic_elements,
        row_env_var,
        padded_source_env_var,
        token_quadratic_env_var,
        planner: "stable_source_byte_len_then_chunk_index".to_string(),
    };
    validate_embedder_batch_controls(embedder, &controls)?;
    Ok(controls)
}

fn embedder_hard_max_rows(embedder: EmbedderId) -> Option<usize> {
    match embedder {
        EmbedderId::E6 | EmbedderId::E7 | EmbedderId::E12 | EmbedderId::E13 => Some(32),
        EmbedderId::E14 => Some(8),
        _ => None,
    }
}

fn default_token_quadratic_elements(
    embedder: EmbedderId,
    global_true_batch_size: usize,
    max_source_bytes: usize,
) -> FsvResult<u64> {
    match embedder {
        EmbedderId::E1
        | EmbedderId::E6
        | EmbedderId::E7
        | EmbedderId::E8
        | EmbedderId::E10
        | EmbedderId::E12
        | EmbedderId::E13
        | EmbedderId::E14 => Ok(DEFAULT_PRETRAINED_MAX_TOKEN_QUADRATIC_ELEMENTS),
        _ => padded_token_quadratic_elements(max_source_bytes, global_true_batch_size),
    }
}

fn validate_embedder_batch_controls(
    embedder: EmbedderId,
    controls: &EmbedderBatchControls,
) -> FsvResult<()> {
    require(
        "batch controls target requested embedder",
        controls.embedder_id == embedder.slug(),
    )?;
    require("per-embedder max rows is non-zero", controls.max_rows > 0)?;
    require(
        "per-embedder padded source-byte budget is non-zero",
        controls.max_padded_source_bytes > 0,
    )?;
    require(
        "per-embedder token-quadratic budget is non-zero",
        controls.max_token_quadratic_elements > 0,
    )?;
    if let Some(hard_max_rows) = controls.hard_max_rows {
        if controls.max_rows > hard_max_rows {
            return Err(format!(
                "MEJEPA_FORWARD_BATCH_CONFIG_INVALID: embedder={} requested_max_rows={} hard_max_rows={}",
                embedder.slug(),
                controls.max_rows,
                hard_max_rows
            )
            .into());
        }
    }
    Ok(())
}

fn plan_pending_batches(
    embedder: EmbedderId,
    pending: &[PendingForward],
    controls: &EmbedderBatchControls,
) -> FsvResult<Vec<Vec<PendingForward>>> {
    validate_embedder_batch_controls(embedder, controls)?;
    if pending.is_empty() {
        return Ok(Vec::new());
    }
    let mut ordered = pending.to_vec();
    ordered.sort_by_key(|pending| (pending.source_byte_len, pending.chunk_index));

    let mut batches = Vec::new();
    let mut current: Vec<PendingForward> = Vec::new();
    for row in ordered {
        let would_exceed_rows = current.len() >= controls.max_rows;
        let would_exceed_source = if current.is_empty() {
            false
        } else {
            let current_max = current
                .iter()
                .map(|pending| pending.source_byte_len)
                .max()
                .ok_or("non-empty planner batch missing source length")?;
            let next_max = current_max.max(row.source_byte_len);
            padded_source_bytes(next_max, current.len() + 1)? > controls.max_padded_source_bytes
        };
        let would_exceed_token_quadratic = if current.is_empty() {
            false
        } else {
            let current_max = current
                .iter()
                .map(|pending| pending.source_byte_len)
                .max()
                .ok_or("non-empty planner batch missing source length")?;
            let next_max = current_max.max(row.source_byte_len);
            padded_token_quadratic_elements(next_max, current.len() + 1)?
                > controls.max_token_quadratic_elements
        };
        if would_exceed_rows || would_exceed_source || would_exceed_token_quadratic {
            require("planner emits only non-empty batches", !current.is_empty())?;
            batches.push(current);
            current = Vec::new();
        }
        current.push(row);
        let current_stats = batch_source_stats(&current)?;
        require(
            "planner batch respects max row control",
            current_stats.row_count <= controls.max_rows,
        )?;
        require(
            "planner batch respects padded source-byte budget unless single row exceeds budget",
            current_stats.padded_source_bytes <= controls.max_padded_source_bytes
                || current_stats.row_count == 1,
        )?;
        require(
            "planner batch respects token-quadratic budget unless single row exceeds budget",
            current_stats.padded_token_quadratic_elements <= controls.max_token_quadratic_elements
                || current_stats.row_count == 1,
        )?;
    }
    if !current.is_empty() {
        batches.push(current);
    }
    require("planner emitted at least one batch", !batches.is_empty())?;
    Ok(batches)
}

fn batch_source_stats(batch: &[PendingForward]) -> FsvResult<BatchSourceStats> {
    require("batch source stats has at least one row", !batch.is_empty())?;
    let mut source_byte_min = usize::MAX;
    let mut source_byte_max = 0usize;
    let mut source_byte_sum = 0usize;
    let mut token_estimate_sum = 0usize;
    for row in batch {
        source_byte_min = source_byte_min.min(row.source_byte_len);
        source_byte_max = source_byte_max.max(row.source_byte_len);
        source_byte_sum = source_byte_sum
            .checked_add(row.source_byte_len)
            .ok_or("batch source byte sum overflow")?;
        token_estimate_sum = token_estimate_sum
            .checked_add(estimated_tokens_from_source_bytes(row.source_byte_len))
            .ok_or("batch token estimate sum overflow")?;
    }
    Ok(BatchSourceStats {
        row_count: batch.len(),
        source_byte_min,
        source_byte_max,
        source_byte_sum,
        padded_source_bytes: padded_source_bytes(source_byte_max, batch.len())?,
        token_estimate_min: estimated_tokens_from_source_bytes(source_byte_min),
        token_estimate_max: estimated_tokens_from_source_bytes(source_byte_max),
        token_estimate_sum,
        padded_token_quadratic_elements: padded_token_quadratic_elements(
            source_byte_max,
            batch.len(),
        )?,
    })
}

fn padded_source_bytes(source_byte_max: usize, row_count: usize) -> FsvResult<u64> {
    (source_byte_max as u64)
        .checked_mul(row_count as u64)
        .ok_or_else(|| "padded source-byte budget overflow".into())
}

fn estimated_tokens_from_source_bytes(source_byte_len: usize) -> usize {
    source_byte_len.saturating_add(3) / 4
}

fn padded_token_quadratic_elements(source_byte_max: usize, row_count: usize) -> FsvResult<u64> {
    let token_max = estimated_tokens_from_source_bytes(source_byte_max) as u64;
    token_max
        .checked_mul(token_max)
        .and_then(|value| value.checked_mul(row_count as u64))
        .ok_or_else(|| "token-quadratic batch budget overflow".into())
}

#[allow(clippy::too_many_arguments)]
async fn first_pass_true_batch_record(
    paths: &Paths,
    config: &ModelsConfig,
    chunks: &[AstChunk],
    record: &RegistryRecord,
    max_source_bytes: usize,
    hot_model_cache_max_bytes: u64,
    batch_controls: &EmbedderBatchControls,
    gpu_vram_guard_bytes: u64,
    cache_store: &mut ForwardCacheStore,
    index_sink: &mut JsonlSink,
    quarantine_sink: &mut JsonlSink,
    batch_telemetry_sink: &mut JsonlSink,
    coverage_tracker: &mut CoverageTracker,
    first_pass_count: &mut usize,
    first_pass_sample: &mut Vec<CacheAction>,
    quarantine_sample: &mut Vec<Value>,
    quarantined_pairs: &mut BTreeSet<String>,
) -> FsvResult<()> {
    let embedder = record.embedder_id()?;
    require(
        "record uses active true-batch forward embedder",
        is_true_batch_forward_embedder(embedder),
    )?;
    validate_embedder_batch_controls(embedder, batch_controls)?;

    let mut pending = Vec::new();
    for (chunk_index, chunk) in chunks.iter().enumerate() {
        if let Some((code, reason)) = chunk_non_forwardable_reason(chunk, max_source_bytes) {
            let row = quarantine_row(chunk, record, code, reason, max_source_bytes)?;
            quarantine_sink.write_row(&row)?;
            coverage_tracker.record_quarantine(record)?;
            record_quarantine_pair(quarantined_pairs, chunk, record)?;
            push_sample(quarantine_sample, row);
            continue;
        }

        let started = Instant::now();
        let key = build_key(chunk, record, max_source_bytes)?;
        let key_sha = key_hash(&key)?;
        let record_locator = cache_store.record_locator(paths, embedder.slug(), &key)?;
        let cache_present = cache_store.contains(paths, embedder.slug(), &key)?;
        write_progress(
            paths,
            chunk,
            record,
            "cache_lookup",
            json!({
                "cache_key_sha256": key_sha,
                "record_path": record_locator.clone(),
                "cache_store_layout": cache_store.layout().as_str(),
                "cache_present": cache_present,
                "true_batch_candidate": true,
            }),
        )?;
        if cache_present {
            let readback = cache_store.read(paths, embedder.slug(), &key)?;
            validate_record(&readback.record, record)?;
            require(
                "cache hit key matches requested key",
                readback.record.cache_key_sha256 == key_sha && readback.record.key == key,
            )?;
            let action = cache_action_from_readback(
                CacheActionContext {
                    embedder,
                    key_sha,
                    action: "hit",
                    true_batch: false,
                    batch_id: None,
                    batch_size: None,
                    batch_index: None,
                    latency_ms: started.elapsed().as_millis(),
                },
                readback,
            );
            index_sink.write_row(&index_row(chunk, record, &action))?;
            coverage_tracker.record_first(record, &action)?;
            *first_pass_count += 1;
            push_sample(first_pass_sample, action);
        } else {
            write_progress(
                paths,
                chunk,
                record,
                "cache_miss",
                json!({
                    "cache_key_sha256": key_sha,
                    "record_path": record_locator.clone(),
                    "cache_store_layout": cache_store.layout().as_str(),
                    "true_batch_candidate": true,
                }),
            )?;
            let input = input_for_chunk(embedder, chunk, max_source_bytes)?;
            let source_byte_len = input.text.len();
            pending.push(PendingForward {
                chunk_index,
                key,
                key_sha,
                path: PathBuf::from(record_locator.clone()),
                input,
                source_byte_len,
            });
        }
    }

    if pending.is_empty() {
        return Ok(());
    }

    let first_pending = &pending[0];
    let first_chunk = &chunks[first_pending.chunk_index];
    let loaded = LoadedForward::load(
        paths,
        config,
        embedder,
        first_chunk,
        record,
        hot_model_cache_max_bytes,
    )
    .await?;
    require(
        "loaded wrapper supports true batch",
        loaded.supports_true_batch(),
    )?;

    let planned_batches = plan_pending_batches(embedder, &pending, batch_controls)?;
    for (batch_index, batch) in planned_batches.iter().enumerate() {
        let batch_started = Instant::now();
        let batch_id = format!("{}-batch-{batch_index:06}", embedder.slug());
        let source_stats = batch_source_stats(batch)?;
        let vram_plan = validate_runner_true_batch_vram_guard(
            embedder,
            batch.len(),
            source_stats.source_byte_max,
            gpu_vram_guard_bytes,
        )?;
        let chunk_ids = batch
            .iter()
            .map(|pending| chunks[pending.chunk_index].chunk_id.clone())
            .collect::<Vec<_>>();
        write_progress(
            paths,
            &chunks[batch[0].chunk_index],
            record,
            "true_batch_forward_start",
            json!({
                "batch_id": batch_id,
                "batch_index": batch_index,
                "batch_size": batch.len(),
                "chunk_ids": chunk_ids,
                "batch_controls": batch_controls,
                "source_stats": source_stats.clone(),
                "vram_plan": vram_plan.clone(),
            }),
        )?;
        let forward_outcome = forward_true_batch_with_token_quarantine(
            &loaded,
            paths,
            chunks,
            record,
            &batch_id,
            batch_index,
            batch,
            max_source_bytes,
            quarantine_sink,
            coverage_tracker,
            quarantine_sample,
            quarantined_pairs,
        )
        .await?;
        require(
            "true-batch outputs plus token quarantines account for pending batch count",
            forward_outcome.outputs.len() + forward_outcome.quarantined_rows.len() == batch.len(),
        )?;

        let mut row_summaries = Vec::with_capacity(forward_outcome.outputs.len());
        let mut precision_classes = BTreeSet::new();
        for (row_index, pending, output) in forward_outcome.outputs {
            let chunk = &chunks[pending.chunk_index];
            let precision_class = output.precision_class.clone();
            precision_classes.insert(precision_class.clone());
            let record_json =
                record_from_output(chunk, record, pending.key, pending.key_sha.clone(), output)?;
            validate_record(&record_json, record)?;
            let readback = cache_store.write(paths, embedder.slug(), &record_json)?;
            require(
                "promoted true-batch record readback key equality",
                readback.record.cache_key_sha256 == record_json.cache_key_sha256
                    && readback.record.key == record_json.key,
            )?;
            require(
                "promoted true-batch record readback vector hash equality",
                readback.record.output.vector_sha256 == record_json.output.vector_sha256
                    && readback.record.output.vector_sha256
                        == vector_sha256(&readback.record.output.vector),
            )?;
            let action = cache_action_from_readback(
                CacheActionContext {
                    embedder,
                    key_sha: pending.key_sha.clone(),
                    action: "write",
                    true_batch: true,
                    batch_id: Some(batch_id.clone()),
                    batch_size: Some(batch.len()),
                    batch_index: Some(batch_index),
                    latency_ms: batch_started.elapsed().as_millis(),
                },
                readback,
            );
            index_sink.write_row(&index_row(chunk, record, &action))?;
            coverage_tracker.record_first(record, &action)?;
            *first_pass_count += 1;
            push_sample(first_pass_sample, action.clone());
            row_summaries.push(json!({
                "row_index": row_index,
                "chunk_id": chunk.chunk_id,
                "cache_key_sha256": pending.key_sha,
                "record_path": action.record_path,
                "record_store_layout": action.record_store_layout,
                "packed_record_path": action.packed_record_path,
                "packed_byte_offset": action.packed_byte_offset,
                "packed_byte_len": action.packed_byte_len,
                "record_sha256": action.record_sha256,
                "vector_sha256": record_json.output.vector_sha256,
                "vector_len": record_json.output.vector_len,
                "precision_class": precision_class,
            }));
        }

        let telemetry = json!({
            "schema_version": CACHE_SCHEMA_VERSION,
            "task_id": "TASK-PY-DATA-039",
            "artifact_kind": "python_forward_true_batch_telemetry",
            "timestamp_utc": utc_now()?,
            "embedder_id": embedder.slug(),
            "instrument_id": record.instrument_id,
            "instrument_record_id": record.instrument_record_id,
            "batch_id": batch_id,
            "batch_index": batch_index,
            "batch_size": batch.len(),
            "batch_controls": batch_controls,
            "source_stats": source_stats,
            "output_count": row_summaries.len(),
            "quarantine_count": forward_outcome.quarantined_rows.len(),
            "true_batch": true,
            "precision_classes": precision_classes.into_iter().collect::<Vec<_>>(),
            "latency_ms": batch_started.elapsed().as_millis(),
            "vram_plan": vram_plan,
            "quarantined_rows": forward_outcome.quarantined_rows,
            "rows": row_summaries,
        });
        batch_telemetry_sink.write_row(&telemetry)?;
        write_progress(
            paths,
            &chunks[batch[0].chunk_index],
            record,
            "true_batch_forward_complete",
            telemetry,
        )?;
    }
    Ok(())
}

#[derive(Debug)]
struct BatchForwardOutcome {
    outputs: Vec<(usize, PendingForward, EmbedderOutput)>,
    quarantined_rows: Vec<Value>,
}

#[allow(clippy::too_many_arguments)]
async fn forward_true_batch_with_token_quarantine(
    loaded: &LoadedForward,
    paths: &Paths,
    chunks: &[AstChunk],
    record: &RegistryRecord,
    batch_id: &str,
    batch_index: usize,
    batch: &[PendingForward],
    max_source_bytes: usize,
    quarantine_sink: &mut JsonlSink,
    coverage_tracker: &mut CoverageTracker,
    quarantine_sample: &mut Vec<Value>,
    quarantined_pairs: &mut BTreeSet<String>,
) -> FsvResult<BatchForwardOutcome> {
    let mut stack: Vec<Vec<(usize, PendingForward)>> =
        vec![batch.iter().cloned().enumerate().collect()];
    let mut outputs = Vec::with_capacity(batch.len());
    let mut quarantined_rows = Vec::new();

    while let Some(rows) = stack.pop() {
        require("true-batch isolation group is non-empty", !rows.is_empty())?;
        let inputs = rows
            .iter()
            .map(|(_, pending)| pending.input.clone())
            .collect::<Vec<_>>();
        match loaded.forward_true_batch(&inputs).await {
            Ok(group_outputs) => {
                require(
                    "isolated true-batch output count matches group count",
                    group_outputs.len() == rows.len(),
                )?;
                for ((row_index, pending), output) in rows.into_iter().zip(group_outputs) {
                    outputs.push((row_index, pending, output));
                }
            }
            Err(error) => {
                let Some(isolation) = classify_true_batch_isolation_error(error.as_ref()) else {
                    return Err(error);
                };
                let error_text = error_chain_text(error.as_ref());
                if rows.len() == 1 {
                    let (row_index, pending) = rows
                        .into_iter()
                        .next()
                        .ok_or("true-batch isolation group unexpectedly empty")?;
                    let chunk = &chunks[pending.chunk_index];
                    let reason = format!(
                        "{} inside true-batch {}; row_index={}; error={}",
                        isolation.quarantine_reason, batch_id, row_index, error_text
                    );
                    let row = quarantine_row(
                        chunk,
                        record,
                        isolation.error_code,
                        &reason,
                        max_source_bytes,
                    )?;
                    quarantine_sink.write_row(&row)?;
                    coverage_tracker.record_quarantine(record)?;
                    record_quarantine_pair(quarantined_pairs, chunk, record)?;
                    push_sample(quarantine_sample, row.clone());
                    write_progress(
                        paths,
                        chunk,
                        record,
                        isolation.quarantine_event,
                        json!({
                            "batch_id": batch_id,
                            "batch_index": batch_index,
                            "row_index": row_index,
                            "cache_key_sha256": pending.key_sha,
                            "record_path": pending.path.display().to_string(),
                            "error_code": isolation.error_code,
                            "error": error_text,
                        }),
                    )?;
                    quarantined_rows.push(row);
                } else {
                    let split_at = rows.len() / 2;
                    require("true-batch split makes progress", split_at > 0)?;
                    let first_chunk = &chunks[rows[0].1.chunk_index];
                    write_progress(
                        paths,
                        first_chunk,
                        record,
                        isolation.split_event,
                        json!({
                            "batch_id": batch_id,
                            "batch_index": batch_index,
                            "group_size": rows.len(),
                            "left_size": split_at,
                            "right_size": rows.len() - split_at,
                            "error_code": isolation.error_code,
                            "error": error_text,
                        }),
                    )?;
                    stack.push(rows[split_at..].to_vec());
                    stack.push(rows[..split_at].to_vec());
                }
            }
        }
    }

    outputs.sort_by_key(|(row_index, _, _)| *row_index);
    Ok(BatchForwardOutcome {
        outputs,
        quarantined_rows,
    })
}

#[derive(Debug, Clone, Copy)]
struct TrueBatchIsolation {
    error_code: &'static str,
    split_event: &'static str,
    quarantine_event: &'static str,
    quarantine_reason: &'static str,
}

fn classify_true_batch_isolation_error(
    error: &dyn std::error::Error,
) -> Option<TrueBatchIsolation> {
    if is_token_overlimit_error(error) {
        return Some(TrueBatchIsolation {
            error_code: "MEJEPA_FORWARD_TOKEN_TOO_LONG",
            split_event: "true_batch_token_overlimit_split",
            quarantine_event: "true_batch_token_overlimit_quarantine",
            quarantine_reason: "model tokenizer rejected this chunk",
        });
    }
    if is_gpu_oom_error(error) {
        return Some(TrueBatchIsolation {
            error_code: "MEJEPA_FORWARD_GPU_OOM",
            split_event: "true_batch_gpu_oom_split",
            quarantine_event: "true_batch_gpu_oom_quarantine",
            quarantine_reason: "GPU memory exhaustion remained after batch isolation",
        });
    }
    None
}

fn is_token_overlimit_error(error: &dyn std::error::Error) -> bool {
    let text = error_chain_text(error);
    text.contains("Input too long") && text.contains("exceeds max")
}

fn is_gpu_oom_error(error: &dyn std::error::Error) -> bool {
    let text = error_chain_text(error).to_ascii_lowercase();
    text.contains("cuda_error_out_of_memory")
        || (text.contains("gpu error") && text.contains("out of memory"))
}

fn error_chain_text(error: &dyn std::error::Error) -> String {
    let mut parts = vec![error.to_string()];
    let mut current = error.source();
    while let Some(source) = current {
        parts.push(source.to_string());
        current = source.source();
    }
    parts.join(" | ")
}

fn validate_runner_true_batch_vram_guard(
    embedder: EmbedderId,
    batch_size: usize,
    source_byte_max: usize,
    guard_bytes: u64,
) -> FsvResult<RunnerBatchVramPlan> {
    require("runner batch size is non-zero", batch_size > 0)?;
    require(
        "runner batch source-byte max is non-zero",
        source_byte_max > 0,
    )?;
    let model_id = embedder_model_id(embedder)?;
    let model_vram_bytes = get_memory_estimate(model_id) as u64;
    let input_budget_bytes = (batch_size as u64)
        .checked_mul(source_byte_max as u64)
        .ok_or("true-batch input byte budget overflow")?;
    let output_budget_bytes = (batch_size as u64)
        .checked_mul(embedder.dimension() as u64)
        .and_then(|value| value.checked_mul(std::mem::size_of::<f32>() as u64))
        .ok_or("true-batch output byte budget overflow")?;
    let padded_token_quadratic_elements =
        padded_token_quadratic_elements(source_byte_max, batch_size)?;
    let attention_budget_bytes =
        true_batch_attention_budget_bytes(embedder, padded_token_quadratic_elements)?;
    let required_bytes = model_vram_bytes
        .checked_add(input_budget_bytes)
        .and_then(|value| value.checked_add(output_budget_bytes))
        .and_then(|value| value.checked_add(attention_budget_bytes))
        .ok_or("true-batch VRAM required byte overflow")?;
    if required_bytes > guard_bytes {
        return Err(format!(
            "MEJEPA_FORWARD_TRUE_BATCH_VRAM_GUARD: embedder={} required_bytes={} guard_bytes={} batch_size={}",
            embedder.slug(),
            required_bytes,
            guard_bytes,
            batch_size
        )
        .into());
    }
    Ok(RunnerBatchVramPlan {
        embedder_id: embedder.slug().to_string(),
        model_id: model_id.as_str().to_string(),
        batch_size,
        source_byte_max,
        model_vram_bytes,
        input_budget_bytes,
        output_budget_bytes,
        attention_budget_bytes,
        padded_token_quadratic_elements,
        required_bytes,
        guard_bytes,
    })
}

fn true_batch_attention_budget_bytes(
    embedder: EmbedderId,
    padded_token_quadratic_elements: u64,
) -> FsvResult<u64> {
    let bytes_per_element = match embedder {
        EmbedderId::E14 => 384,
        EmbedderId::E7 => 256,
        EmbedderId::E1
        | EmbedderId::E6
        | EmbedderId::E8
        | EmbedderId::E10
        | EmbedderId::E12
        | EmbedderId::E13 => 128,
        _ => 0,
    };
    padded_token_quadratic_elements
        .checked_mul(bytes_per_element)
        .ok_or_else(|| "true-batch attention budget overflow".into())
}

async fn cache_or_forward(
    paths: &Paths,
    config: &ModelsConfig,
    chunk: &AstChunk,
    record: &RegistryRecord,
    loaded: &mut Option<LoadedForward>,
    max_source_bytes: usize,
    hot_model_cache_max_bytes: u64,
    cache_store: &mut ForwardCacheStore,
) -> FsvResult<CacheAction> {
    let started = Instant::now();
    let key = build_key(chunk, record, max_source_bytes)?;
    let key_sha = key_hash(&key)?;
    let embedder = record.embedder_id()?;
    let record_locator = cache_store.record_locator(paths, embedder.slug(), &key)?;
    let cache_present = cache_store.contains(paths, embedder.slug(), &key)?;
    write_progress(
        paths,
        chunk,
        record,
        "cache_lookup",
        json!({
            "cache_key_sha256": key_sha,
            "record_path": record_locator.clone(),
            "cache_store_layout": cache_store.layout().as_str(),
            "cache_present": cache_present,
        }),
    )?;
    if cache_present {
        write_progress(
            paths,
            chunk,
            record,
            "cache_hit_read_start",
            json!({
                "cache_key_sha256": key_sha,
                "record_path": record_locator.clone(),
                "cache_store_layout": cache_store.layout().as_str(),
            }),
        )?;
        let readback = cache_store.read(paths, embedder.slug(), &key)?;
        validate_record(&readback.record, record)?;
        require(
            "cache hit key matches requested key",
            readback.record.cache_key_sha256 == key_sha && readback.record.key == key,
        )?;
        write_progress(
            paths,
            chunk,
            record,
            "cache_hit_complete",
            json!({
                "cache_key_sha256": key_sha,
                "record_path": readback.record_path,
                "record_store_layout": readback.record_store_layout,
                "packed_record_path": readback.packed_record_path,
                "packed_byte_offset": readback.packed_byte_offset,
                "packed_byte_len": readback.packed_byte_len,
                "vector_sha256": readback.record.output.vector_sha256,
                "vector_len": readback.record.output.vector_len,
                "record_sha256": readback.record_sha256,
                "latency_ms": started.elapsed().as_millis(),
            }),
        )?;
        return Ok(cache_action_from_readback(
            CacheActionContext {
                embedder,
                key_sha,
                action: "hit",
                true_batch: false,
                batch_id: None,
                batch_size: None,
                batch_index: None,
                latency_ms: started.elapsed().as_millis(),
            },
            readback,
        ));
    }

    write_progress(
        paths,
        chunk,
        record,
        "cache_miss",
        json!({
            "cache_key_sha256": key_sha,
            "record_path": record_locator.clone(),
            "cache_store_layout": cache_store.layout().as_str(),
        }),
    )?;
    let input = input_for_chunk(embedder, chunk, max_source_bytes)?;
    if loaded.is_none() {
        write_progress(
            paths,
            chunk,
            record,
            "load_start",
            json!({
                    "cache_key_sha256": key_sha,
                    "record_path": record_locator.clone(),
                    "cache_store_layout": cache_store.layout().as_str(),
                    "input_sha256": sha256_prefixed_bytes(input.text.as_bytes()),
                "hot_model_cache_root": paths.hot_model_cache_root().display().to_string(),
                "hot_model_cache_max_bytes": hot_model_cache_max_bytes,
            }),
        )?;
        *loaded = Some(
            LoadedForward::load(
                paths,
                config,
                embedder,
                chunk,
                record,
                hot_model_cache_max_bytes,
            )
            .await?,
        );
        write_progress(
            paths,
            chunk,
            record,
            "load_complete",
            json!({
                    "cache_key_sha256": key_sha,
                    "record_path": record_locator.clone(),
                    "cache_store_layout": cache_store.layout().as_str(),
                    "input_sha256": sha256_prefixed_bytes(input.text.as_bytes()),
                "latency_ms": started.elapsed().as_millis(),
            }),
        )?;
    } else {
        write_progress(
            paths,
            chunk,
            record,
            "load_reuse",
            json!({
                    "cache_key_sha256": key_sha,
                    "record_path": record_locator.clone(),
                    "cache_store_layout": cache_store.layout().as_str(),
                    "input_sha256": sha256_prefixed_bytes(input.text.as_bytes()),
            }),
        )?;
    }
    write_progress(
        paths,
        chunk,
        record,
        "forward_start",
        json!({
            "cache_key_sha256": key_sha,
            "record_path": record_locator.clone(),
            "cache_store_layout": cache_store.layout().as_str(),
            "input_sha256": sha256_prefixed_bytes(input.text.as_bytes()),
        }),
    )?;
    let output = loaded
        .as_ref()
        .ok_or("loaded forwarder missing after load")?
        .forward(&input)
        .await?;
    write_progress(
        paths,
        chunk,
        record,
        "forward_complete",
        json!({
            "cache_key_sha256": key_sha,
            "record_path": record_locator.clone(),
            "vector_len": output.vector.len(),
            "vector_sha256": vector_sha256(&output.vector),
            "runtime_model_version": output.model_version,
            "latency_ms": started.elapsed().as_millis(),
        }),
    )?;
    let record_json = record_from_output(chunk, record, key, key_sha.clone(), output)?;
    validate_record(&record_json, record)?;
    write_progress(
        paths,
        chunk,
        record,
        "write_start",
        json!({
            "cache_key_sha256": key_sha,
            "record_path": record_locator.clone(),
            "cache_store_layout": cache_store.layout().as_str(),
            "vector_sha256": record_json.output.vector_sha256,
            "vector_len": record_json.output.vector_len,
        }),
    )?;
    let readback = cache_store.write(paths, embedder.slug(), &record_json)?;
    require(
        "written record readback key equality",
        readback.record.cache_key_sha256 == record_json.cache_key_sha256
            && readback.record.key == record_json.key,
    )?;
    require(
        "written record readback vector hash equality",
        readback.record.output.vector_sha256 == record_json.output.vector_sha256
            && readback.record.output.vector_sha256
                == vector_sha256(&readback.record.output.vector),
    )?;
    let action = cache_action_from_readback(
        CacheActionContext {
            embedder,
            key_sha: key_sha.clone(),
            action: "write",
            true_batch: false,
            batch_id: None,
            batch_size: None,
            batch_index: None,
            latency_ms: started.elapsed().as_millis(),
        },
        readback,
    );
    write_progress(
        paths,
        chunk,
        record,
        "write_complete",
        json!({
            "cache_key_sha256": key_sha,
            "record_path": action.record_path,
            "record_store_layout": action.record_store_layout,
            "packed_record_path": action.packed_record_path,
            "packed_byte_offset": action.packed_byte_offset,
            "packed_byte_len": action.packed_byte_len,
            "record_sha256": action.record_sha256,
            "vector_sha256": record_json.output.vector_sha256,
            "vector_len": record_json.output.vector_len,
            "byte_count": action.byte_count,
            "latency_ms": started.elapsed().as_millis(),
        }),
    )?;
    Ok(action)
}

fn write_progress(
    paths: &Paths,
    chunk: &AstChunk,
    record: &RegistryRecord,
    phase: &str,
    detail: Value,
) -> FsvResult<()> {
    write_json_checked(
        &paths.progress_path(),
        &json!({
            "schema_version": CACHE_SCHEMA_VERSION,
            "task_id": "TASK-PY-DATA-016",
            "artifact_kind": "python_ast_chunk_forward_progress",
            "timestamp_utc": utc_now()?,
            "phase": phase,
            "embedder_id": record.embedder_id()?.slug(),
            "instrument_id": record.instrument_id,
            "instrument_record_id": record.instrument_record_id,
            "chunk_id": chunk.chunk_id,
            "source_sha256": chunk.source_sha256,
            "file_sha256": chunk.file_sha256,
            "task_instance_id": chunk.task_instance_id,
            "repo": chunk.repo,
            "workspace_state": chunk.workspace_state,
            "absolute_path": chunk.absolute_path,
            "relative_path": chunk.relative_path,
            "source_text_bytes": chunk.source_text.len(),
            "hot_progress_path": paths.progress_path().display().to_string(),
            "durable_cache_root": paths.cache_root.display().to_string(),
            "detail": detail,
        }),
    )
}

fn record_from_output(
    chunk: &AstChunk,
    registry: &RegistryRecord,
    key: ForwardCacheKey,
    key_sha: String,
    output: EmbedderOutput,
) -> FsvResult<ForwardCacheRecord> {
    let embedder = registry.embedder_id()?;
    require(
        "output embedder matches registry",
        output.embedder == embedder,
    )?;
    let vector_sha = vector_sha256(&output.vector);
    Ok(ForwardCacheRecord {
        schema_version: CACHE_SCHEMA_VERSION,
        cache_key_sha256: key_sha,
        key,
        chunk: ChunkRef {
            task_instance_id: chunk.task_instance_id.clone(),
            repo: chunk.repo.clone(),
            workspace_state: chunk.workspace_state.clone(),
            absolute_path: chunk.absolute_path.clone(),
            relative_path: chunk.relative_path.clone(),
            chunk_time: chunk.time.clone(),
        },
        instrument_id: registry.instrument_id.clone(),
        instrument_kind: registry.instrument_kind.clone(),
        output: ForwardCacheOutput {
            vector_len: output.vector.len(),
            vector_sha256: vector_sha,
            vector_l2_norm: output.vector_l2_norm(),
            precision_class: output.precision_class,
            runtime_model_version: output.model_version,
            vector: output.vector,
        },
        source_readback_verified: true,
    })
}

fn build_key(
    chunk: &AstChunk,
    record: &RegistryRecord,
    max_source_bytes: usize,
) -> FsvResult<ForwardCacheKey> {
    let embedder = record.embedder_id()?;
    let input = input_for_chunk(embedder, chunk, max_source_bytes)?;
    Ok(ForwardCacheKey {
        schema_version: CACHE_SCHEMA_VERSION,
        artifact_kind: "python_ast_chunk_forward".to_string(),
        chunk_id: chunk.chunk_id.clone(),
        source_sha256: chunk.source_sha256.clone(),
        file_sha256: chunk.file_sha256.clone(),
        embedder_id: embedder.slug().to_string(),
        instrument_record_id: record.instrument_record_id.clone(),
        preprocessing_hash: record.preprocessing_hash.clone(),
        model_hash: record.model_hash()?,
        input_sha256: sha256_prefixed_bytes(input.text.as_bytes()),
        output_feature_family: record.output_feature_family.clone(),
    })
}

fn input_for_chunk(
    embedder: EmbedderId,
    chunk: &AstChunk,
    max_source_bytes: usize,
) -> FsvResult<EmbedderInput> {
    if chunk.source_text.trim().is_empty() {
        return Err("AST chunk source_text is empty; refusing to forward an empty chunk".into());
    }
    if chunk.source_text.len() > max_source_bytes {
        return Err(format!(
            "AST chunk source_text has {} bytes, exceeding max forward guard of {} bytes",
            chunk.source_text.len(),
            max_source_bytes
        )
        .into());
    }
    let text = if matches!(embedder, EmbedderId::E2 | EmbedderId::E3 | EmbedderId::E4) {
        format!("timestamp:{}\n{}", chunk.time, chunk.source_text)
    } else {
        chunk.source_text.clone()
    };
    Ok(EmbedderInput {
        embedder,
        text,
        source_id: chunk.absolute_path.clone(),
    })
}

fn load_chunks(path: &Path, chunk_offset: usize, max_chunks: usize) -> FsvResult<Vec<AstChunk>> {
    let file = File::open(path)?;
    let mut out = Vec::new();
    let mut accepted_index = 0usize;
    for line in BufReader::new(file).lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let chunk: AstChunk = serde_json::from_str(&line)?;
        require(
            "chunk_id is sha256-prefixed",
            is_prefixed_sha256(&chunk.chunk_id),
        )?;
        require(
            "source_sha256 is sha256-prefixed",
            is_prefixed_sha256(&chunk.source_sha256),
        )?;
        if accepted_index < chunk_offset {
            accepted_index += 1;
            continue;
        }
        out.push(chunk);
        if out.len() == max_chunks {
            break;
        }
        accepted_index += 1;
    }
    require(
        "at least one AST chunk loaded from requested offset/window",
        !out.is_empty(),
    )?;
    Ok(out)
}

fn load_active_embedder_records(
    path: &Path,
    only_embedder: Option<EmbedderId>,
) -> FsvResult<Vec<RegistryRecord>> {
    let file = File::open(path)?;
    let mut out = Vec::new();
    for line in BufReader::new(file).lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let record: RegistryRecord = serde_json::from_str(&line)?;
        if record.activation_status == "active" && record.panel_role == "target_side_embedder" {
            validate_registry_record(&record)?;
            let embedder = record.embedder_id()?;
            if only_embedder.map_or(true, |only| only == embedder) {
                out.push(record);
            }
        }
    }
    out.sort_by_key(|record| {
        record
            .embedder_id()
            .map(|id| id.slug().to_string())
            .unwrap_or_default()
    });
    require(
        "filtered active embedder registry is non-empty",
        !out.is_empty(),
    )?;
    Ok(out)
}

fn validate_registry_record(record: &RegistryRecord) -> FsvResult<()> {
    let embedder = record.embedder_id()?;
    require(
        "registry record is target-side frozen",
        record.target_side_frozen,
    )?;
    require(
        "registry record has zero trainable parameters",
        record.trainable_parameter_count == 0,
    )?;
    require(
        "registry dimension matches embedder dimension",
        record.output_dimension == Some(embedder.dimension()),
    )?;
    require(
        "registry preprocessing hash is sha256-prefixed",
        is_prefixed_sha256(&record.preprocessing_hash),
    )?;
    record.model_hash()?;
    Ok(())
}

fn verify_registry_matches_runtime(
    registry: &[RegistryRecord],
    only_embedder: Option<EmbedderId>,
) -> FsvResult<()> {
    let registry_ids = registry
        .iter()
        .map(RegistryRecord::embedder_id)
        .collect::<Result<BTreeSet<_>, _>>()?;
    if let Some(only) = only_embedder {
        require(
            "single-embedder process isolation registry contains exactly requested embedder",
            registry_ids == BTreeSet::from([only]),
        )?;
        require(
            "requested isolated embedder is supported by forward runtime",
            SUPPORTED_FORWARD_EMBEDDERS.contains(&only),
        )?;
        return Ok(());
    }
    let runtime_ids = SUPPORTED_FORWARD_EMBEDDERS
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    require(
        "registry active embedders exactly match supported forward embedders",
        registry_ids == runtime_ids,
    )?;
    Ok(())
}

fn validate_record(record: &ForwardCacheRecord, registry: &RegistryRecord) -> FsvResult<()> {
    let embedder = registry.embedder_id()?;
    require(
        "record schema version matches",
        record.schema_version == CACHE_SCHEMA_VERSION,
    )?;
    require(
        "record source readback verified",
        record.source_readback_verified,
    )?;
    require(
        "record key hash matches payload",
        record.cache_key_sha256 == key_hash(&record.key)?,
    )?;
    require(
        "record embedder matches registry",
        record.key.embedder_id == embedder.slug(),
    )?;
    require(
        "record preprocessing hash matches registry",
        record.key.preprocessing_hash == registry.preprocessing_hash,
    )?;
    require(
        "record model hash matches registry",
        record.key.model_hash == registry.model_hash()?,
    )?;
    require(
        "record full vector dimension matches registry",
        record.output.vector_len == registry.output_dimension.unwrap_or_default()
            && record.output.vector.len() == registry.output_dimension.unwrap_or_default(),
    )?;
    require(
        "record vector contains no NaN/Inf",
        record.output.vector.iter().all(|value| value.is_finite()),
    )?;
    require(
        "record vector hash matches full vector bytes",
        record.output.vector_sha256 == vector_sha256(&record.output.vector),
    )?;
    require(
        "record vector is non-zero",
        record.output.vector_l2_norm > 0.0,
    )?;
    Ok(())
}

fn read_cache_record(path: &Path) -> FsvResult<ForwardCacheRecord> {
    let bytes = fs::read(path)?;
    let record: ForwardCacheRecord = serde_json::from_slice(&bytes)?;
    Ok(record)
}

struct ForwardCacheStore {
    layout: CacheStoreLayout,
    packed_index: BTreeMap<String, PackedIndexRow>,
}

impl ForwardCacheStore {
    fn open(paths: &Paths, layout: CacheStoreLayout) -> FsvResult<Self> {
        if layout == CacheStoreLayout::PackedJsonl {
            fs::create_dir_all(paths.packed_root())?;
        }
        let mut store = Self {
            layout,
            packed_index: BTreeMap::new(),
        };
        if layout == CacheStoreLayout::PackedJsonl {
            store.packed_index = load_packed_index_rows(paths)?;
        }
        Ok(store)
    }

    fn layout(&self) -> CacheStoreLayout {
        self.layout
    }

    fn record_locator(
        &self,
        paths: &Paths,
        embedder: &str,
        key: &ForwardCacheKey,
    ) -> FsvResult<String> {
        match self.layout {
            CacheStoreLayout::LegacyFiles => {
                Ok(cache_record_path(&paths.cache_root, embedder, key)?
                    .display()
                    .to_string())
            }
            CacheStoreLayout::PackedJsonl => Ok(format!(
                "{}#{}",
                paths.packed_record_path(embedder).display(),
                key_hash(key)?
            )),
        }
    }

    fn contains(&self, paths: &Paths, embedder: &str, key: &ForwardCacheKey) -> FsvResult<bool> {
        match self.layout {
            CacheStoreLayout::LegacyFiles => {
                Ok(cache_record_path(&paths.cache_root, embedder, key)?.is_file())
            }
            CacheStoreLayout::PackedJsonl => Ok(self.packed_index.contains_key(&key_hash(key)?)),
        }
    }

    fn read(
        &self,
        paths: &Paths,
        embedder: &str,
        key: &ForwardCacheKey,
    ) -> FsvResult<CacheRecordReadback> {
        let key_sha = key_hash(key)?;
        match self.layout {
            CacheStoreLayout::LegacyFiles => {
                let path = cache_record_path(&paths.cache_root, embedder, key)?;
                let bytes = fs::read(&path)?;
                let record: ForwardCacheRecord = serde_json::from_slice(&bytes)?;
                Ok(CacheRecordReadback {
                    record,
                    record_path: path.display().to_string(),
                    record_store_layout: self.layout.as_str().to_string(),
                    packed_record_path: None,
                    packed_index_path: None,
                    packed_byte_offset: None,
                    packed_byte_len: None,
                    record_sha256: sha256_prefixed_bytes(&bytes),
                    byte_count: bytes.len() as u64,
                })
            }
            CacheStoreLayout::PackedJsonl => {
                let row = self
                    .packed_index
                    .get(&key_sha)
                    .ok_or_else(|| format!("missing packed index row for {key_sha}"))?;
                let bytes = read_packed_payload_bytes(row)?;
                require(
                    "packed payload sha256 matches index row",
                    sha256_prefixed_bytes(&bytes) == row.record_sha256,
                )?;
                let record: ForwardCacheRecord = serde_json::from_slice(&bytes)?;
                Ok(CacheRecordReadback {
                    record,
                    record_path: format!("{}#{}", row.packed_record_path, row.cache_key_sha256),
                    record_store_layout: self.layout.as_str().to_string(),
                    packed_record_path: Some(row.packed_record_path.clone()),
                    packed_index_path: Some(
                        paths.packed_index_path(embedder).display().to_string(),
                    ),
                    packed_byte_offset: Some(row.byte_offset),
                    packed_byte_len: Some(row.byte_len),
                    record_sha256: row.record_sha256.clone(),
                    byte_count: row.byte_len,
                })
            }
        }
    }

    fn write(
        &mut self,
        paths: &Paths,
        embedder: &str,
        record: &ForwardCacheRecord,
    ) -> FsvResult<CacheRecordReadback> {
        match self.layout {
            CacheStoreLayout::LegacyFiles => {
                let path = cache_record_path(&paths.cache_root, embedder, &record.key)?;
                write_cache_record_promoted_checked(paths, &path, record)?;
                self.read(paths, embedder, &record.key)
            }
            CacheStoreLayout::PackedJsonl => {
                require(
                    "packed cache duplicate key rejected before append",
                    !self.packed_index.contains_key(&record.cache_key_sha256),
                )?;
                require(
                    "packed cache root is durable D-drive",
                    paths.packed_root().starts_with("/mnt/d"),
                )?;
                fs::create_dir_all(paths.packed_root())?;
                let packed_record_path = paths.packed_record_path(embedder);
                let packed_index_path = paths.packed_index_path(embedder);
                let bytes = serde_json::to_vec(record)?;
                let mut file = OpenOptions::new()
                    .create(true)
                    .append(true)
                    .read(true)
                    .open(&packed_record_path)?;
                let byte_offset = file.metadata()?.len();
                file.write_all(&bytes)?;
                file.write_all(b"\n")?;
                file.flush()?;
                let row = PackedIndexRow {
                    schema_version: CACHE_SCHEMA_VERSION,
                    artifact_kind: "python_forward_cache_packed_index_row".to_string(),
                    cache_key_sha256: record.cache_key_sha256.clone(),
                    embedder_id: embedder.to_string(),
                    packed_record_path: packed_record_path.display().to_string(),
                    byte_offset,
                    byte_len: bytes.len() as u64,
                    record_sha256: sha256_prefixed_bytes(&bytes),
                    vector_sha256: record.output.vector_sha256.clone(),
                    vector_len: record.output.vector_len,
                };
                append_jsonl_row_checked(&packed_index_path, &row)?;
                self.packed_index
                    .insert(record.cache_key_sha256.clone(), row.clone());
                let readback = self.read(paths, embedder, &record.key)?;
                require(
                    "packed write readback key equality",
                    readback.record.cache_key_sha256 == record.cache_key_sha256
                        && readback.record.key == record.key,
                )?;
                require(
                    "packed write readback vector hash equality",
                    readback.record.output.vector_sha256 == record.output.vector_sha256
                        && readback.record.output.vector_sha256
                            == vector_sha256(&readback.record.output.vector),
                )?;
                Ok(readback)
            }
        }
    }

    fn write_manifest(&self, paths: &Paths) -> FsvResult<Value> {
        let mut per_embedder = BTreeMap::new();
        if paths.packed_root().exists() {
            for entry in fs::read_dir(paths.packed_root())? {
                let path = entry?.path();
                if !is_packed_payload_path(&path) {
                    continue;
                }
                let embedder = path
                    .file_stem()
                    .and_then(|name| name.to_str())
                    .ok_or("packed payload file has invalid stem")?
                    .to_string();
                let index_path = paths.packed_index_path(&embedder);
                let rows = read_packed_index_file(&index_path)?;
                per_embedder.insert(
                    embedder,
                    json!({
                        "record_path": path.display().to_string(),
                        "record_sha256": sha256_file(&path)?,
                        "byte_count": fs::metadata(&path)?.len(),
                        "index_path": index_path.display().to_string(),
                        "index_sha256": sha256_file(&index_path)?,
                        "row_count": rows.len(),
                    }),
                );
            }
        }
        let total_rows = self.packed_index.len();
        let manifest = json!({
            "schema_version": CACHE_SCHEMA_VERSION,
            "artifact_kind": "python_forward_cache_packed_manifest",
            "created_at_utc": utc_now()?,
            "layout": self.layout.as_str(),
            "packed_record_root": paths.packed_root().display().to_string(),
            "row_count": total_rows,
            "per_embedder": per_embedder,
            "passes": true,
        });
        write_json_checked(&paths.packed_manifest_path(), &manifest)?;
        let readback: Value = serde_json::from_slice(&fs::read(paths.packed_manifest_path())?)?;
        require(
            "packed manifest readback row count matches",
            readback["row_count"].as_u64() == Some(total_rows as u64),
        )?;
        Ok(manifest)
    }
}

fn read_packed_payload_bytes(row: &PackedIndexRow) -> FsvResult<Vec<u8>> {
    require("packed index byte length is positive", row.byte_len > 0)?;
    let mut file = File::open(&row.packed_record_path)?;
    file.seek(SeekFrom::Start(row.byte_offset))?;
    let mut bytes = vec![0u8; row.byte_len as usize];
    file.read_exact(&mut bytes)?;
    Ok(bytes)
}

fn load_packed_index_rows(paths: &Paths) -> FsvResult<BTreeMap<String, PackedIndexRow>> {
    let mut rows = BTreeMap::new();
    if !paths.packed_root().exists() {
        return Ok(rows);
    }
    for entry in fs::read_dir(paths.packed_root())? {
        let path = entry?.path();
        if !is_packed_index_path(&path) {
            continue;
        }
        for row in read_packed_index_file(&path)? {
            let inserted = rows.insert(row.cache_key_sha256.clone(), row).is_none();
            require("packed index duplicate key rejected on load", inserted)?;
        }
    }
    Ok(rows)
}

fn read_packed_index_file(path: &Path) -> FsvResult<Vec<PackedIndexRow>> {
    if !path.is_file() {
        return Ok(Vec::new());
    }
    let file = File::open(path)?;
    let mut rows = Vec::new();
    let mut seen = BTreeSet::new();
    for line in BufReader::new(file).lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let row: PackedIndexRow = serde_json::from_str(&line)?;
        require(
            "packed index key appears once per file",
            seen.insert(row.cache_key_sha256.clone()),
        )?;
        require(
            "packed index row hash matches payload bytes",
            sha256_prefixed_bytes(&read_packed_payload_bytes(&row)?) == row.record_sha256,
        )?;
        rows.push(row);
    }
    Ok(rows)
}

fn write_cache_record_promoted_checked(
    paths: &Paths,
    path: &Path,
    record: &ForwardCacheRecord,
) -> FsvResult<Value> {
    require(
        "durable cache record path is on D drive",
        path.starts_with("/mnt/d"),
    )?;
    let bytes = serde_json::to_vec(record)?;
    let stage_path = paths
        .record_stage_root()
        .join(&record.key.embedder_id)
        .join(format!(
            "{}.json",
            record.cache_key_sha256.trim_start_matches("sha256:")
        ));
    write_bytes_atomic(&stage_path, &bytes)?;
    require(
        "hot-stage cache record byte readback equality",
        fs::read(&stage_path)? == bytes,
    )?;
    write_bytes_atomic(path, &bytes)?;
    require(
        "cache record byte readback equality",
        fs::read(path)? == bytes,
    )?;
    let readback: ForwardCacheRecord = serde_json::from_slice(&fs::read(path)?)?;
    require(
        "cache record readback key equality",
        readback.cache_key_sha256 == record.cache_key_sha256 && readback.key == record.key,
    )?;
    require(
        "cache record readback vector hash equality",
        readback.output.vector_sha256 == record.output.vector_sha256
            && readback.output.vector_sha256 == vector_sha256(&readback.output.vector),
    )?;
    require(
        "cache record readback vector length equality",
        readback.output.vector.len() == record.output.vector.len(),
    )?;
    Ok(json!({
        "stage_path": stage_path.display().to_string(),
        "stage_sha256": sha256_file(&stage_path)?,
        "durable_path": path.display().to_string(),
        "durable_sha256": sha256_file(path)?,
        "byte_count": bytes.len(),
        "stage_readback_equals_payload": true,
        "durable_readback_equals_stage": fs::read(path)? == fs::read(&stage_path)?,
    }))
}

fn cache_record_path(root: &Path, embedder: &str, key: &ForwardCacheKey) -> FsvResult<PathBuf> {
    Ok(root.join("records").join(embedder).join(format!(
        "{}.json",
        key_hash(key)?.trim_start_matches("sha256:")
    )))
}

fn index_row(chunk: &AstChunk, record: &RegistryRecord, action: &CacheAction) -> Value {
    json!({
        "schema_version": CACHE_SCHEMA_VERSION,
        "task_id": "TASK-PY-DATA-016",
        "cache_key_sha256": action.cache_key_sha256,
        "record_path": action.record_path,
        "record_store_layout": action.record_store_layout,
        "packed_record_path": action.packed_record_path,
        "packed_index_path": action.packed_index_path,
        "packed_byte_offset": action.packed_byte_offset,
        "packed_byte_len": action.packed_byte_len,
        "record_sha256": action.record_sha256,
        "vector_sha256": action.vector_sha256,
        "vector_len": action.vector_len,
        "latency_ms": action.latency_ms,
        "true_batch": action.true_batch,
        "batch_id": action.batch_id,
        "batch_size": action.batch_size,
        "batch_index": action.batch_index,
        "embedder_id": action.embedder_id,
        "instrument_id": record.instrument_id,
        "chunk_id": chunk.chunk_id,
        "source_sha256": chunk.source_sha256,
        "file_sha256": chunk.file_sha256,
        "task_instance_id": chunk.task_instance_id,
        "workspace_state": chunk.workspace_state,
    })
}

fn quarantine_pair_key(chunk: &AstChunk, record: &RegistryRecord) -> FsvResult<String> {
    Ok(format!(
        "{}:{}",
        record.embedder_id()?.slug(),
        chunk.chunk_id
    ))
}

fn record_quarantine_pair(
    quarantined_pairs: &mut BTreeSet<String>,
    chunk: &AstChunk,
    record: &RegistryRecord,
) -> FsvResult<()> {
    let pair_key = quarantine_pair_key(chunk, record)?;
    require(
        "quarantine pair is recorded exactly once",
        quarantined_pairs.insert(pair_key),
    )
}

fn quarantine_row(
    chunk: &AstChunk,
    record: &RegistryRecord,
    error_code: &str,
    reason: &str,
    max_source_bytes: usize,
) -> FsvResult<Value> {
    Ok(json!({
        "schema_version": CACHE_SCHEMA_VERSION,
        "task_id": "TASK-PY-DATA-016",
        "artifact_kind": "python_ast_chunk_forward_quarantine",
        "error_code": error_code,
        "reason": reason,
        "embedder_id": record.embedder_id()?.slug(),
        "instrument_id": record.instrument_id,
        "instrument_record_id": record.instrument_record_id,
        "chunk_id": chunk.chunk_id,
        "source_sha256": chunk.source_sha256,
        "file_sha256": chunk.file_sha256,
        "task_instance_id": chunk.task_instance_id,
        "repo": chunk.repo,
        "workspace_state": chunk.workspace_state,
        "absolute_path": chunk.absolute_path,
        "relative_path": chunk.relative_path,
        "source_text_bytes": chunk.source_text.len(),
        "max_forward_source_text_bytes": max_source_bytes,
    }))
}

fn chunk_non_forwardable_reason(
    chunk: &AstChunk,
    max_source_bytes: usize,
) -> Option<(&'static str, &'static str)> {
    if chunk.source_text.trim().is_empty() {
        Some((
            "MEJEPA_FORWARD_EMPTY_CHUNK",
            "accepted Python AST chunk source_text is empty or whitespace-only",
        ))
    } else if chunk.source_text.len() > max_source_bytes {
        Some((
            "MEJEPA_FORWARD_INPUT_TOO_LARGE",
            "accepted Python AST chunk source_text exceeds max forward byte guard",
        ))
    } else {
        None
    }
}

fn push_sample<T>(sample: &mut Vec<T>, value: T) {
    if sample.len() < SAMPLE_LIMIT {
        sample.push(value);
    }
}

impl CoverageTracker {
    fn new(registry: &[RegistryRecord], expected_chunks: usize) -> FsvResult<Self> {
        let mut by_embedder = BTreeMap::new();
        for record in registry {
            by_embedder.insert(record.embedder_id()?.slug().to_string(), Default::default());
        }
        Ok(Self {
            expected_chunks,
            by_embedder,
        })
    }

    fn record_first(&mut self, record: &RegistryRecord, action: &CacheAction) -> FsvResult<()> {
        let stats = self.stats_mut(record)?;
        match action.action.as_str() {
            "hit" => stats.first_pass_hits += 1,
            "write" => stats.first_pass_writes += 1,
            other => {
                return Err(format!("unknown first-pass cache action {other}").into());
            }
        }
        stats.first_pass_latency_ms_total += action.latency_ms;
        stats.vector_lens.insert(action.vector_len);
        Ok(())
    }

    fn record_second(&mut self, record: &RegistryRecord, action: &CacheAction) -> FsvResult<()> {
        let stats = self.stats_mut(record)?;
        require("second pass action is a cache hit", action.action == "hit")?;
        stats.second_pass_hits += 1;
        stats.second_pass_latency_ms_total += action.latency_ms;
        stats.vector_lens.insert(action.vector_len);
        Ok(())
    }

    fn record_quarantine(&mut self, record: &RegistryRecord) -> FsvResult<()> {
        self.stats_mut(record)?.quarantined_pairs += 1;
        Ok(())
    }

    fn first_pass_hits(&self) -> usize {
        self.by_embedder
            .values()
            .map(|stats| stats.first_pass_hits)
            .sum()
    }

    fn first_pass_writes(&self) -> usize {
        self.by_embedder
            .values()
            .map(|stats| stats.first_pass_writes)
            .sum()
    }

    fn second_pass_hits(&self) -> usize {
        self.by_embedder
            .values()
            .map(|stats| stats.second_pass_hits)
            .sum()
    }

    fn quarantined_pairs(&self) -> usize {
        self.by_embedder
            .values()
            .map(|stats| stats.quarantined_pairs)
            .sum()
    }

    fn summary(&self, registry: &[RegistryRecord]) -> FsvResult<Vec<Value>> {
        let mut rows = Vec::new();
        for record in registry {
            let embedder = record.embedder_id()?.slug().to_string();
            let stats = self
                .by_embedder
                .get(&embedder)
                .ok_or_else(|| format!("missing coverage stats for {embedder}"))?;
            let first_outputs = stats.first_pass_hits + stats.first_pass_writes;
            require(
                "coverage first pass plus quarantine count matches chunk count",
                first_outputs + stats.quarantined_pairs == self.expected_chunks,
            )?;
            require(
                "coverage second pass hit count matches first-pass output count",
                stats.second_pass_hits == first_outputs,
            )?;
            require(
                "coverage has at most one dimension when outputs exist",
                stats.vector_lens.len() <= 1,
            )?;
            let output_coverage = if self.expected_chunks == 0 {
                0.0
            } else {
                first_outputs as f64 / self.expected_chunks as f64
            };
            rows.push(json!({
                "embedder_id": embedder,
                "expected_chunks": self.expected_chunks,
                "forwarded_chunks": first_outputs,
                "first_pass_hits": stats.first_pass_hits,
                "first_pass_writes": stats.first_pass_writes,
                "first_pass_failures": stats.first_pass_failures,
                "second_pass_hits": stats.second_pass_hits,
                "quarantined_pairs": stats.quarantined_pairs,
                "output_coverage_ratio": output_coverage,
                "accounted_pair_ratio": 1.0,
                "vector_len": stats.vector_lens.iter().next().copied(),
                "latency_ms": {
                    "first_pass_total": stats.first_pass_latency_ms_total,
                    "second_pass_total": stats.second_pass_latency_ms_total,
                }
            }));
        }
        Ok(rows)
    }

    fn stats_mut(&mut self, record: &RegistryRecord) -> FsvResult<&mut EmbedderCoverageStats> {
        let embedder = record.embedder_id()?.slug().to_string();
        self.by_embedder
            .get_mut(&embedder)
            .ok_or_else(|| format!("missing coverage stats for {embedder}").into())
    }
}

fn edge_not_applicable(paths: &Paths, case: &str, reason: &str) -> FsvResult<Value> {
    let before = store_state(&format!("{case}_before"), paths)?;
    let after = store_state(&format!("{case}_after"), paths)?;
    require(
        "not-applicable edge writes no cache records",
        before.cache_record_count == after.cache_record_count,
    )?;
    Ok(json!({
        "case": case,
        "before_state": before,
        "trigger": reason,
        "after_state": after,
        "not_applicable": true,
        "passes": true,
    }))
}

fn edge_empty_chunk(paths: &Paths, registry: &RegistryRecord) -> FsvResult<Value> {
    let before = store_state("edge_empty_before", paths)?;
    let mut chunk = load_chunks(&paths.chunks_jsonl, 0, 1)?.remove(0);
    chunk.source_text.clear();
    let err = build_key(&chunk, registry, DEFAULT_MAX_FORWARD_SOURCE_TEXT_BYTES).unwrap_err();
    let quarantine = quarantine_row(
        &chunk,
        registry,
        "MEJEPA_FORWARD_EMPTY_CHUNK",
        "accepted Python AST chunk source_text is empty or whitespace-only",
        DEFAULT_MAX_FORWARD_SOURCE_TEXT_BYTES,
    )?;
    let evidence_path = paths.hot_root.join("edge-empty-quarantine.json");
    write_json_checked(&evidence_path, &quarantine)?;
    let evidence_readback: Value = serde_json::from_slice(&fs::read(&evidence_path)?)?;
    require(
        "empty chunk quarantine evidence readback error code",
        evidence_readback["error_code"] == Value::String("MEJEPA_FORWARD_EMPTY_CHUNK".to_string()),
    )?;
    let after = store_state("edge_empty_after", paths)?;
    require(
        "empty chunk edge writes no cache records",
        before.cache_record_count == after.cache_record_count,
    )?;
    Ok(json!({
        "case": "empty_chunk_rejected",
        "before_state": before,
        "trigger": "attempt to build a forward-cache key for an empty AST chunk source_text",
        "after_state": after,
        "error": err.to_string(),
        "quarantine_evidence_path": evidence_path.display().to_string(),
        "quarantine_evidence_sha256": sha256_file(&evidence_path)?,
        "quarantine_evidence_readback": evidence_readback,
        "passes": true,
    }))
}

async fn edge_embedder_mismatch(paths: &Paths) -> FsvResult<Value> {
    let before = store_state("edge_mismatch_before", paths)?;
    let forward = AlgorithmicEmbedderForward::load(EmbedderId::E9)?;
    let err = forward
        .forward(&EmbedderInput {
            embedder: EmbedderId::E2,
            text: "timestamp:2026-05-14T00:00:00+00:00\nx = 1".to_string(),
            source_id: "synthetic/mismatch.py".to_string(),
        })
        .await
        .unwrap_err();
    let after = store_state("edge_mismatch_after", paths)?;
    require(
        "embedder mismatch edge writes no cache records",
        before.cache_record_count == after.cache_record_count,
    )?;
    Ok(json!({
        "case": "embedder_mismatch_rejected",
        "before_state": before,
        "trigger": "E9 wrapper receives an E2 input",
        "after_state": after,
        "error_code": err.code(),
        "error": err.to_string(),
        "passes": true,
    }))
}

async fn edge_unsupported_true_batch(paths: &Paths) -> FsvResult<Value> {
    let before = store_state("edge_unsupported_true_batch_before", paths)?;
    let forward = AlgorithmicEmbedderForward::load(EmbedderId::E2)?;
    let err = forward
        .forward_true_batch(&[EmbedderInput {
            embedder: EmbedderId::E2,
            text: "timestamp:2026-05-14T00:00:00+00:00\ndef temporal_row():\n    return 1\n"
                .to_string(),
            source_id: "synthetic/unsupported_true_batch.py".to_string(),
        }])
        .await
        .unwrap_err();
    let after = store_state("edge_unsupported_true_batch_after", paths)?;
    require(
        "unsupported true-batch edge writes no cache records",
        before.cache_record_count == after.cache_record_count,
    )?;
    Ok(json!({
        "case": "unsupported_true_batch_rejected",
        "before_state": before,
        "trigger": "request true-batch forwarding from E2 temporal algorithmic wrapper before exact-output CUDA proof exists",
        "after_state": after,
        "error_code": err.code(),
        "error": err.to_string(),
        "passes": true,
    }))
}

fn edge_invalid_embedder_batch_config(paths: &Paths) -> FsvResult<Value> {
    let before = store_state("edge_invalid_embedder_batch_config_before", paths)?;
    let cases = [
        (EmbedderId::E7, 64usize),
        (EmbedderId::E12, 33usize),
        (EmbedderId::E14, 16usize),
    ];
    let mut rejections = Vec::new();
    for (embedder, max_rows) in cases {
        let slug_upper = embedder.slug().to_ascii_uppercase();
        let controls = EmbedderBatchControls {
            embedder_id: embedder.slug().to_string(),
            max_rows,
            hard_max_rows: embedder_hard_max_rows(embedder),
            max_padded_source_bytes: (max_rows as u64) * 32_768,
            max_token_quadratic_elements: default_token_quadratic_elements(
                embedder, max_rows, 32_768,
            )?,
            row_env_var: format!("CG_MEJEPA_PY_FORWARD_TRUE_BATCH_SIZE_{slug_upper}"),
            padded_source_env_var: format!(
                "CG_MEJEPA_PY_FORWARD_TRUE_BATCH_MAX_PADDED_SOURCE_BYTES_{slug_upper}"
            ),
            token_quadratic_env_var: format!(
                "CG_MEJEPA_PY_FORWARD_TRUE_BATCH_MAX_TOKEN_QUADRATIC_{slug_upper}"
            ),
            planner: "stable_source_byte_len_then_chunk_index".to_string(),
        };
        let err = validate_embedder_batch_controls(embedder, &controls).unwrap_err();
        rejections.push(json!({
            "embedder_id": embedder.slug(),
            "requested_max_rows": max_rows,
            "hard_max_rows": embedder_hard_max_rows(embedder),
            "batch_controls": controls,
            "error": err.to_string(),
        }));
    }
    let after = store_state("edge_invalid_embedder_batch_config_after", paths)?;
    require(
        "invalid embedder batch config writes no cache records",
        before.cache_record_count == after.cache_record_count,
    )?;
    Ok(json!({
        "case": "invalid_embedder_batch_config_rejected",
        "before_state": before,
        "trigger": "request hard-max violations for E7, E12, and E14",
        "rejections": rejections,
        "after_state": after,
        "passes": true,
    }))
}

fn edge_batch_planner_source_budget(paths: &Paths) -> FsvResult<Value> {
    let before = store_state("edge_batch_planner_source_budget_before", paths)?;
    let controls = EmbedderBatchControls {
        embedder_id: EmbedderId::E14.slug().to_string(),
        max_rows: 8,
        hard_max_rows: embedder_hard_max_rows(EmbedderId::E14),
        max_padded_source_bytes: 64,
        max_token_quadratic_elements: DEFAULT_E14_MAX_TOKEN_QUADRATIC_ELEMENTS,
        row_env_var: "CG_MEJEPA_PY_FORWARD_TRUE_BATCH_SIZE_E14".to_string(),
        padded_source_env_var: "CG_MEJEPA_PY_FORWARD_TRUE_BATCH_MAX_PADDED_SOURCE_BYTES_E14"
            .to_string(),
        token_quadratic_env_var: "CG_MEJEPA_PY_FORWARD_TRUE_BATCH_MAX_TOKEN_QUADRATIC_E14"
            .to_string(),
        planner: "stable_source_byte_len_then_chunk_index".to_string(),
    };
    let pending = [8usize, 16, 17, 32, 64]
        .into_iter()
        .enumerate()
        .map(|(chunk_index, source_byte_len)| {
            edge_pending_forward(EmbedderId::E14, chunk_index, source_byte_len)
        })
        .collect::<FsvResult<Vec<_>>>()?;
    let batches = plan_pending_batches(EmbedderId::E14, &pending, &controls)?;
    let stats = batches
        .iter()
        .map(|batch| batch_source_stats(batch))
        .collect::<FsvResult<Vec<_>>>()?;
    require(
        "source-budget planner splits synthetic long chunks",
        batches.len() > 1,
    )?;
    require(
        "source-budget planner preserves every pending row",
        batches.iter().map(Vec::len).sum::<usize>() == pending.len(),
    )?;
    require(
        "source-budget planner never exceeds row limit",
        stats.iter().all(|stat| stat.row_count <= controls.max_rows),
    )?;
    require(
        "source-budget planner only permits over-budget singleton rows",
        stats.iter().all(|stat| {
            stat.padded_source_bytes <= controls.max_padded_source_bytes || stat.row_count == 1
        }),
    )?;
    let after = store_state("edge_batch_planner_source_budget_after", paths)?;
    require(
        "source-budget planner edge writes no cache records",
        before.cache_record_count == after.cache_record_count,
    )?;
    Ok(json!({
        "case": "source_budget_batch_planner_splits_long_rows",
        "before_state": before,
        "trigger": "plan synthetic E14 pending rows under a 64-byte padded source budget",
        "batch_controls": controls,
        "batch_count": batches.len(),
        "batch_stats": stats,
        "after_state": after,
        "passes": true,
    }))
}

fn edge_token_quadratic_batch_planner(paths: &Paths) -> FsvResult<Value> {
    let before = store_state("edge_token_quadratic_batch_planner_before", paths)?;
    let controls = EmbedderBatchControls {
        embedder_id: EmbedderId::E14.slug().to_string(),
        max_rows: 4,
        hard_max_rows: embedder_hard_max_rows(EmbedderId::E14),
        max_padded_source_bytes: 262_144,
        max_token_quadratic_elements: DEFAULT_E14_MAX_TOKEN_QUADRATIC_ELEMENTS,
        row_env_var: "CG_MEJEPA_PY_FORWARD_TRUE_BATCH_SIZE_E14".to_string(),
        padded_source_env_var: "CG_MEJEPA_PY_FORWARD_TRUE_BATCH_MAX_PADDED_SOURCE_BYTES_E14"
            .to_string(),
        token_quadratic_env_var: "CG_MEJEPA_PY_FORWARD_TRUE_BATCH_MAX_TOKEN_QUADRATIC_E14"
            .to_string(),
        planner: "stable_source_byte_len_then_chunk_index".to_string(),
    };
    let pending = [
        1_221usize, 1_424, 1_454, 1_531, 2_472, 3_845, 14_496, 20_510,
    ]
    .into_iter()
    .enumerate()
    .map(|(chunk_index, source_byte_len)| {
        edge_pending_forward(EmbedderId::E14, chunk_index, source_byte_len)
    })
    .collect::<FsvResult<Vec<_>>>()?;
    let batches = plan_pending_batches(EmbedderId::E14, &pending, &controls)?;
    let stats = batches
        .iter()
        .map(|batch| batch_source_stats(batch))
        .collect::<FsvResult<Vec<_>>>()?;
    require(
        "token-quadratic planner preserves every pending row",
        batches.iter().map(Vec::len).sum::<usize>() == pending.len(),
    )?;
    require(
        "token-quadratic planner isolates longest rows instead of padded batching them",
        stats
            .iter()
            .filter(|stat| stat.source_byte_max >= 14_496)
            .all(|stat| stat.row_count == 1),
    )?;
    require(
        "token-quadratic planner never exceeds budget unless singleton",
        stats.iter().all(|stat| {
            stat.padded_token_quadratic_elements <= controls.max_token_quadratic_elements
                || stat.row_count == 1
        }),
    )?;
    let after = store_state("edge_token_quadratic_batch_planner_after", paths)?;
    require(
        "token-quadratic planner edge writes no cache records",
        before.cache_record_count == after.cache_record_count,
    )?;
    Ok(json!({
        "case": "token_quadratic_batch_planner_isolates_e14_long_rows",
        "before_state": before,
        "trigger": "plan synthetic E14 shard-5 length distribution under a quadratic token budget",
        "batch_controls": controls,
        "batch_count": batches.len(),
        "batch_stats": stats,
        "after_state": after,
        "passes": true,
    }))
}

fn edge_pretrained_token_quadratic_defaults(paths: &Paths) -> FsvResult<Value> {
    let before = store_state("edge_pretrained_token_quadratic_defaults_before", paths)?;
    let global_true_batch_size = 8usize;
    let max_source_bytes = 32_768usize;
    let mut pretrained = Vec::new();
    for embedder in SUPPORTED_PRETRAINED_FORWARD_EMBEDDERS {
        let token_budget =
            default_token_quadratic_elements(*embedder, global_true_batch_size, max_source_bytes)?;
        let attention_budget = true_batch_attention_budget_bytes(*embedder, token_budget)?;
        require(
            "pretrained transformer default token-quadratic budget is bounded",
            token_budget == DEFAULT_PRETRAINED_MAX_TOKEN_QUADRATIC_ELEMENTS,
        )?;
        require(
            "pretrained transformer default attention budget is non-zero",
            attention_budget > 0,
        )?;
        pretrained.push(json!({
            "embedder_id": embedder.slug(),
            "token_quadratic_budget": token_budget,
            "attention_budget_bytes": attention_budget,
        }));
    }
    let e9_budget =
        default_token_quadratic_elements(EmbedderId::E9, global_true_batch_size, max_source_bytes)?;
    require(
        "E9 algorithmic HDC keeps source-derived token-quadratic default",
        e9_budget == padded_token_quadratic_elements(max_source_bytes, global_true_batch_size)?,
    )?;
    require(
        "E9 algorithmic HDC default remains larger than transformer guard",
        e9_budget > DEFAULT_PRETRAINED_MAX_TOKEN_QUADRATIC_ELEMENTS,
    )?;
    let after = store_state("edge_pretrained_token_quadratic_defaults_after", paths)?;
    require(
        "pretrained token-quadratic defaults edge writes no cache records",
        before.cache_record_count == after.cache_record_count,
    )?;
    Ok(json!({
        "case": "pretrained_token_quadratic_defaults_bound_transformer_batches",
        "before_state": before,
        "trigger": "inspect default token-quadratic and attention VRAM budgets for all true-batch embedder classes",
        "pretrained": pretrained,
        "algorithmic_e9": {
            "embedder_id": EmbedderId::E9.slug(),
            "token_quadratic_budget": e9_budget,
            "attention_budget_bytes": true_batch_attention_budget_bytes(EmbedderId::E9, e9_budget)?,
        },
        "after_state": after,
        "passes": true,
    }))
}

fn edge_pending_forward(
    embedder: EmbedderId,
    chunk_index: usize,
    source_byte_len: usize,
) -> FsvResult<PendingForward> {
    let text = "x".repeat(source_byte_len);
    Ok(PendingForward {
        chunk_index,
        key: ForwardCacheKey {
            schema_version: CACHE_SCHEMA_VERSION,
            artifact_kind: "edge_python_ast_chunk_forward".to_string(),
            chunk_id: format!("edge-chunk-{chunk_index}"),
            source_sha256: sha256_prefixed_bytes(text.as_bytes()),
            file_sha256: sha256_prefixed_bytes(format!("file-{chunk_index}").as_bytes()),
            embedder_id: embedder.slug().to_string(),
            instrument_record_id: "edge-instrument-record".to_string(),
            preprocessing_hash: sha256_prefixed_bytes(b"edge-preprocessing"),
            model_hash: sha256_prefixed_bytes(b"edge-model"),
            input_sha256: sha256_prefixed_bytes(text.as_bytes()),
            output_feature_family: "edge".to_string(),
        },
        key_sha: sha256_prefixed_bytes(format!("edge-key-{chunk_index}").as_bytes()),
        path: PathBuf::from(format!("/nonexistent/edge/{chunk_index}.json")),
        input: EmbedderInput {
            embedder,
            text,
            source_id: format!("edge/{chunk_index}.py"),
        },
        source_byte_len,
    })
}

fn edge_true_batch_vram_guard(
    paths: &Paths,
    registry: &[RegistryRecord],
    max_source_bytes: usize,
) -> FsvResult<Value> {
    let before = store_state("edge_true_batch_vram_guard_before", paths)?;
    let record = registry
        .iter()
        .find(|record| {
            record
                .embedder_id()
                .is_ok_and(|embedder| SUPPORTED_PRETRAINED_FORWARD_EMBEDDERS.contains(&embedder))
        })
        .ok_or("active registry missing pretrained embedder for VRAM guard edge")?;
    let embedder = record.embedder_id()?;
    let plan = validate_runner_true_batch_vram_guard(embedder, 1, max_source_bytes, u64::MAX)?;
    let err = validate_runner_true_batch_vram_guard(
        embedder,
        1,
        max_source_bytes,
        plan.required_bytes.saturating_sub(1),
    )
    .unwrap_err();
    let after = store_state("edge_true_batch_vram_guard_after", paths)?;
    require(
        "true-batch VRAM guard edge writes no cache records",
        before.cache_record_count == after.cache_record_count,
    )?;
    Ok(json!({
        "case": "true_batch_vram_guard_rejected",
        "before_state": before,
        "trigger": "validate runner true-batch VRAM plan with guard one byte below required bytes",
        "embedder_id": embedder.slug(),
        "required_bytes": plan.required_bytes,
        "guard_bytes": plan.required_bytes.saturating_sub(1),
        "after_state": after,
        "error": err.to_string(),
        "passes": true,
    }))
}

fn edge_true_batch_gpu_oom_classifier(paths: &Paths) -> FsvResult<Value> {
    let before = store_state("edge_true_batch_gpu_oom_classifier_before", paths)?;
    let synthetic_error = std::io::Error::other(
        "true-batch forward pass failed: GPU error: BgeM3DenseModel true-batch layer 0 softmax failed: DriverError(CUDA_ERROR_OUT_OF_MEMORY, \"out of memory\")",
    );
    let isolation = classify_true_batch_isolation_error(&synthetic_error)
        .ok_or("CUDA OOM classifier did not classify synthetic error")?;
    require(
        "CUDA OOM classifier maps to explicit quarantine code",
        isolation.error_code == "MEJEPA_FORWARD_GPU_OOM",
    )?;
    require(
        "CUDA OOM classifier exposes split event",
        isolation.split_event == "true_batch_gpu_oom_split",
    )?;
    let after = store_state("edge_true_batch_gpu_oom_classifier_after", paths)?;
    require(
        "CUDA OOM classifier edge writes no cache records",
        before.cache_record_count == after.cache_record_count,
    )?;
    Ok(json!({
        "case": "true_batch_gpu_oom_classifier_rejects_fail_open",
        "before_state": before,
        "trigger": "classify synthetic Candle CUDA_ERROR_OUT_OF_MEMORY from BGE-M3 softmax",
        "error": synthetic_error.to_string(),
        "error_code": isolation.error_code,
        "split_event": isolation.split_event,
        "quarantine_event": isolation.quarantine_event,
        "after_state": after,
        "passes": true,
    }))
}

fn edge_hot_stage_promotion_mismatch(paths: &Paths) -> FsvResult<Value> {
    let before = store_state("edge_hot_stage_promotion_before", paths)?;
    let stage_path = paths
        .record_stage_root()
        .join("edge-promotion")
        .join("stage.json");
    let durable_path = paths.cache_root.join("edge-promotion-mismatch.json");
    let expected = br#"{"stage":"good","vector_sha256":"sha256:synthetic"}"#;
    let corrupt = br#"{"stage":"corrupt","vector_sha256":"sha256:synthetic"}"#;
    write_bytes_atomic(&stage_path, expected)?;
    require(
        "hot-stage promotion edge stage readback matches expected bytes",
        fs::read(&stage_path)? == expected,
    )?;
    write_bytes_atomic(&durable_path, corrupt)?;
    let mismatch_detected = fs::read(&durable_path)? != fs::read(&stage_path)?;
    require(
        "hot-stage promotion mismatch is detected",
        mismatch_detected,
    )?;
    let after = store_state("edge_hot_stage_promotion_after", paths)?;
    require(
        "hot-stage promotion mismatch edge writes no cache records",
        before.cache_record_count == after.cache_record_count,
    )?;
    Ok(json!({
        "case": "hot_stage_promotion_mismatch_rejected",
        "before_state": before,
        "trigger": "stage valid bytes on WSL ext4, write different bytes to D-drive edge target, then compare SoT readback",
        "stage_path": stage_path.display().to_string(),
        "stage_sha256": sha256_file(&stage_path)?,
        "durable_path": durable_path.display().to_string(),
        "durable_sha256": sha256_file(&durable_path)?,
        "after_state": after,
        "mismatch_detected": mismatch_detected,
        "passes": true,
    }))
}

fn edge_dimension_mismatch(
    paths: &Paths,
    chunk: &AstChunk,
    registry: &RegistryRecord,
    max_source_bytes: usize,
) -> FsvResult<Value> {
    let before = store_state("edge_dimension_before", paths)?;
    let key = build_key(chunk, registry, max_source_bytes)?;
    let bad = ForwardCacheRecord {
        schema_version: CACHE_SCHEMA_VERSION,
        cache_key_sha256: key_hash(&key)?,
        key,
        chunk: ChunkRef {
            task_instance_id: chunk.task_instance_id.clone(),
            repo: chunk.repo.clone(),
            workspace_state: chunk.workspace_state.clone(),
            absolute_path: chunk.absolute_path.clone(),
            relative_path: chunk.relative_path.clone(),
            chunk_time: chunk.time.clone(),
        },
        instrument_id: registry.instrument_id.clone(),
        instrument_kind: registry.instrument_kind.clone(),
        output: ForwardCacheOutput {
            vector: vec![1.0, 2.0],
            vector_len: 2,
            vector_sha256: vector_sha256(&[1.0, 2.0]),
            vector_l2_norm: (5.0_f64).sqrt(),
            precision_class: "synthetic_bad_dimension".to_string(),
            runtime_model_version: "synthetic".to_string(),
        },
        source_readback_verified: true,
    };
    let err = validate_record(&bad, registry).unwrap_err();
    let after = store_state("edge_dimension_after", paths)?;
    require(
        "dimension mismatch edge writes no cache records",
        before.cache_record_count == after.cache_record_count,
    )?;
    Ok(json!({
        "case": "dimension_mismatch_rejected",
        "before_state": before,
        "trigger": "attempt to validate a cache record whose vector length is 2 for the selected embedder",
        "after_state": after,
        "error": err.to_string(),
        "passes": true,
    }))
}

fn edge_cache_corruption(paths: &Paths) -> FsvResult<Value> {
    let before = store_state("edge_cache_corruption_before", paths)?;
    let corrupt_path = paths.hot_root.join("edge-corrupt-cache-record.json");
    write_bytes_atomic(
        &corrupt_path,
        br#"{"schema_version":1,"output":"truncated""#,
    )?;
    let err = read_cache_record(&corrupt_path).unwrap_err();
    let after = store_state("edge_cache_corruption_after", paths)?;
    require(
        "cache corruption edge writes no durable cache records",
        before.cache_record_count == after.cache_record_count,
    )?;
    Ok(json!({
        "case": "cache_corruption_rejected",
        "before_state": before,
        "trigger": "attempt to read malformed JSON from the hot-stage cache edge file",
        "corrupt_path": corrupt_path.display().to_string(),
        "corrupt_file_sha256": sha256_file(&corrupt_path)?,
        "after_state": after,
        "error": err.to_string(),
        "passes": true,
    }))
}

fn edge_paths_with_cache_root(paths: &Paths, cache_root: PathBuf) -> Paths {
    Paths {
        chunks_jsonl: paths.chunks_jsonl.clone(),
        registry_jsonl: paths.registry_jsonl.clone(),
        models_config: paths.models_config.clone(),
        cache_root,
        fsv_root: paths.fsv_root.clone(),
        hot_root: paths.hot_root.clone(),
    }
}

fn write_edge_packed_row(
    paths: &Paths,
    case: &str,
    key: &str,
    payload: &[u8],
    record_sha256_override: Option<String>,
    byte_len_override: Option<u64>,
    missing_payload: bool,
) -> FsvResult<Paths> {
    let cache_root = paths.fsv_root.join("packed_store_edges").join(case);
    if cache_root.exists() {
        fs::remove_dir_all(&cache_root)?;
    }
    let edge_paths = edge_paths_with_cache_root(paths, cache_root);
    fs::create_dir_all(edge_paths.packed_root())?;
    let record_path = edge_paths.packed_record_path("e1");
    let index_path = edge_paths.packed_index_path("e1");
    if !missing_payload {
        let mut record_bytes = payload.to_vec();
        record_bytes.push(b'\n');
        write_bytes_atomic(&record_path, &record_bytes)?;
    }
    let row = PackedIndexRow {
        schema_version: CACHE_SCHEMA_VERSION,
        artifact_kind: "python_forward_cache_packed_index_row".to_string(),
        cache_key_sha256: key.to_string(),
        embedder_id: "e1".to_string(),
        packed_record_path: record_path.display().to_string(),
        byte_offset: 0,
        byte_len: byte_len_override.unwrap_or(payload.len() as u64),
        record_sha256: record_sha256_override.unwrap_or_else(|| sha256_prefixed_bytes(payload)),
        vector_sha256: sha256_prefixed_bytes(b"edge-vector"),
        vector_len: 3,
    };
    append_jsonl_row_checked(&index_path, &row)?;
    Ok(edge_paths)
}

fn edge_packed_duplicate_key(paths: &Paths) -> FsvResult<Value> {
    let before = store_state("edge_packed_duplicate_before", paths)?;
    let key = format!("sha256:{}", "a".repeat(64));
    let edge_paths = write_edge_packed_row(
        paths,
        "duplicate-key",
        &key,
        br#"{"edge":"duplicate"}"#,
        None,
        None,
        false,
    )?;
    let row = read_packed_index_file(&edge_paths.packed_index_path("e1"))?
        .into_iter()
        .next()
        .ok_or("duplicate edge did not create an index row")?;
    append_jsonl_row_checked(&edge_paths.packed_index_path("e1"), &row)?;
    let err = load_packed_index_rows(&edge_paths).unwrap_err();
    let after = store_state("edge_packed_duplicate_after", paths)?;
    require(
        "packed duplicate-key edge writes no primary cache records",
        before.cache_record_count == after.cache_record_count,
    )?;
    Ok(json!({
        "case": "packed_duplicate_key_rejected",
        "before_state": before,
        "trigger": "write two packed index rows with the same cache_key_sha256 and load the packed index",
        "edge_index_path": edge_paths.packed_index_path("e1").display().to_string(),
        "error": err.to_string(),
        "after_state": after,
        "passes": true,
    }))
}

fn edge_packed_corrupt_row(paths: &Paths) -> FsvResult<Value> {
    let before = store_state("edge_packed_corrupt_before", paths)?;
    let key = format!("sha256:{}", "b".repeat(64));
    let edge_paths = write_edge_packed_row(
        paths,
        "corrupt-row",
        &key,
        br#"{"edge":"corrupt"}"#,
        Some(format!("sha256:{}", "0".repeat(64))),
        None,
        false,
    )?;
    let err = load_packed_index_rows(&edge_paths).unwrap_err();
    let after = store_state("edge_packed_corrupt_after", paths)?;
    require(
        "packed corrupt-row edge writes no primary cache records",
        before.cache_record_count == after.cache_record_count,
    )?;
    Ok(json!({
        "case": "packed_corrupt_row_rejected",
        "before_state": before,
        "trigger": "packed index row points to payload bytes whose sha256 does not match record_sha256",
        "edge_index_path": edge_paths.packed_index_path("e1").display().to_string(),
        "error": err.to_string(),
        "after_state": after,
        "passes": true,
    }))
}

fn edge_packed_missing_payload(paths: &Paths) -> FsvResult<Value> {
    let before = store_state("edge_packed_missing_before", paths)?;
    let key = format!("sha256:{}", "c".repeat(64));
    let edge_paths = write_edge_packed_row(
        paths,
        "missing-payload",
        &key,
        br#"{"edge":"missing"}"#,
        None,
        None,
        true,
    )?;
    let err = load_packed_index_rows(&edge_paths).unwrap_err();
    let after = store_state("edge_packed_missing_after", paths)?;
    require(
        "packed missing-payload edge writes no primary cache records",
        before.cache_record_count == after.cache_record_count,
    )?;
    Ok(json!({
        "case": "packed_missing_indexed_payload_rejected",
        "before_state": before,
        "trigger": "packed index row points to an absent payload file",
        "edge_index_path": edge_paths.packed_index_path("e1").display().to_string(),
        "error": err.to_string(),
        "after_state": after,
        "passes": true,
    }))
}

fn edge_packed_truncated_payload(paths: &Paths) -> FsvResult<Value> {
    let before = store_state("edge_packed_truncated_before", paths)?;
    let key = format!("sha256:{}", "d".repeat(64));
    let edge_paths = write_edge_packed_row(
        paths,
        "truncated-payload",
        &key,
        br#"{"edge":"short"}"#,
        None,
        Some(4096),
        false,
    )?;
    let err = load_packed_index_rows(&edge_paths).unwrap_err();
    let after = store_state("edge_packed_truncated_after", paths)?;
    require(
        "packed truncated-payload edge writes no primary cache records",
        before.cache_record_count == after.cache_record_count,
    )?;
    Ok(json!({
        "case": "packed_truncated_payload_rejected",
        "before_state": before,
        "trigger": "packed index row byte_len exceeds available payload bytes",
        "edge_index_path": edge_paths.packed_index_path("e1").display().to_string(),
        "error": err.to_string(),
        "after_state": after,
        "passes": true,
    }))
}

async fn edge_maximum_size_chunk(
    paths: &Paths,
    registry: &[RegistryRecord],
    max_source_bytes: usize,
) -> FsvResult<Value> {
    let before = store_state("edge_maximum_size_before", paths)?;
    let max_scan = load_max_source_chunk(&paths.chunks_jsonl)?;
    let chunk = max_scan.chunk;
    let record = registry
        .iter()
        .find(|record| matches!(record.embedder_id(), Ok(EmbedderId::E9)))
        .ok_or("active registry missing E9 maximum-size edge embedder")?;
    if let Some((code, reason)) = chunk_non_forwardable_reason(&chunk, max_source_bytes) {
        let row = quarantine_row(&chunk, record, code, reason, max_source_bytes)?;
        let evidence_path = paths.hot_root.join("edge-maximum-size-quarantine.json");
        let evidence = json!({
            "case": "maximum_size_chunk_quarantined",
            "quarantine_row": row,
            "scanned_chunk_count": max_scan.scanned,
            "skipped_empty_chunk_count": max_scan.skipped_empty,
        });
        write_json_checked(&evidence_path, &evidence)?;
        let readback: Value = serde_json::from_slice(&fs::read(&evidence_path)?)?;
        require(
            "maximum-size quarantine evidence readback matches error code",
            readback["quarantine_row"]["error_code"] == Value::String(code.to_string()),
        )?;
        let after = store_state("edge_maximum_size_after", paths)?;
        require(
            "maximum-size quarantine edge writes no durable cache records",
            before.cache_record_count == after.cache_record_count,
        )?;
        return Ok(json!({
            "case": "maximum_size_chunk_quarantined",
            "before_state": before,
            "trigger": "stream full accepted Python AST chunk corpus, select largest source_text chunk, and apply fail-closed forwardability guard",
            "evidence_path": evidence_path.display().to_string(),
            "evidence_sha256": sha256_file(&evidence_path)?,
            "evidence_readback": readback,
            "after_state": after,
            "passes": true,
        }));
    }
    let input = input_for_chunk(EmbedderId::E9, &chunk, max_source_bytes)?;
    let started = Instant::now();
    let output = AlgorithmicEmbedderForward::load(EmbedderId::E9)?
        .forward(&input)
        .await?;
    output.validate()?;
    require(
        "maximum-size edge vector dimension matches registry",
        output.vector.len() == record.output_dimension.unwrap_or_default(),
    )?;
    require(
        "maximum-size edge vector is finite",
        output.vector.iter().all(|value| value.is_finite()),
    )?;
    let evidence_path = paths.hot_root.join("edge-maximum-size-e9-forward.json");
    let evidence = json!({
        "case": "maximum_size_chunk_forwarded",
        "embedder_id": EmbedderId::E9.slug(),
        "chunk_id": chunk.chunk_id,
        "source_sha256": chunk.source_sha256,
        "file_sha256": chunk.file_sha256,
        "task_instance_id": chunk.task_instance_id,
        "repo": chunk.repo,
        "relative_path": chunk.relative_path,
        "source_text_bytes": chunk.source_text.len(),
        "scanned_chunk_count": max_scan.scanned,
        "skipped_empty_chunk_count": max_scan.skipped_empty,
        "input_sha256": sha256_prefixed_bytes(input.text.as_bytes()),
        "vector_len": output.vector.len(),
        "vector_sha256": vector_sha256(&output.vector),
        "vector_l2_norm": output.vector_l2_norm(),
        "runtime_model_version": output.model_version,
        "latency_ms": started.elapsed().as_millis(),
    });
    write_json_checked(&evidence_path, &evidence)?;
    let readback: Value = serde_json::from_slice(&fs::read(&evidence_path)?)?;
    require(
        "maximum-size edge evidence readback matches vector hash",
        readback["vector_sha256"] == evidence["vector_sha256"],
    )?;
    require(
        "maximum-size edge evidence readback matches chunk id",
        readback["chunk_id"] == evidence["chunk_id"],
    )?;
    let after = store_state("edge_maximum_size_after", paths)?;
    require(
        "maximum-size edge writes no durable cache records",
        before.cache_record_count == after.cache_record_count,
    )?;
    Ok(json!({
        "case": "maximum_size_chunk_forwarded",
        "before_state": before,
        "trigger": "stream full accepted Python AST chunk corpus, select largest source_text chunk, and forward it through E9",
        "evidence_path": evidence_path.display().to_string(),
        "evidence_sha256": sha256_file(&evidence_path)?,
        "evidence_readback": readback,
        "scanned_chunk_count": max_scan.scanned,
        "skipped_empty_chunk_count": max_scan.skipped_empty,
        "after_state": after,
        "passes": true,
    }))
}

struct MaxChunkScan {
    chunk: AstChunk,
    scanned: usize,
    skipped_empty: usize,
}

fn load_max_source_chunk(path: &Path) -> FsvResult<MaxChunkScan> {
    let file = File::open(path)?;
    let mut best = None;
    let mut seen = 0usize;
    let mut skipped_empty = 0usize;
    for line in BufReader::new(file).lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let chunk: AstChunk = serde_json::from_str(&line)?;
        require(
            "max chunk candidate chunk_id is sha256-prefixed",
            is_prefixed_sha256(&chunk.chunk_id),
        )?;
        require(
            "max chunk candidate source_sha256 is sha256-prefixed",
            is_prefixed_sha256(&chunk.source_sha256),
        )?;
        seen += 1;
        if chunk.source_text.trim().is_empty() {
            skipped_empty += 1;
            continue;
        }
        let replace = best
            .as_ref()
            .map(|current: &AstChunk| chunk.source_text.len() > current.source_text.len())
            .unwrap_or(true);
        if replace {
            best = Some(chunk);
        }
    }
    require("maximum-size scan saw at least one chunk", seen > 0)?;
    let chunk = best.ok_or("maximum-size scan produced no non-empty chunk")?;
    Ok(MaxChunkScan {
        chunk,
        scanned: seen,
        skipped_empty,
    })
}

fn store_state(label: &str, paths: &Paths) -> FsvResult<StoreState> {
    let mut file_count = 0usize;
    let mut byte_count = 0u64;
    let mut record_count = 0usize;
    let mut legacy_record_file_count = 0usize;
    let mut packed_record_row_count = 0usize;
    let mut packed_record_file_count = 0usize;
    let mut record_hash_sample = Vec::new();
    if paths.cache_root.exists() {
        visit_files(&paths.cache_root, &mut |path| {
            file_count += 1;
            byte_count += fs::metadata(path)?.len();
            if path.extension().and_then(|ext| ext.to_str()) == Some("json")
                && path
                    .components()
                    .any(|component| component.as_os_str() == "records")
            {
                record_count += 1;
                legacy_record_file_count += 1;
                if record_hash_sample.len() < SAMPLE_LIMIT {
                    record_hash_sample.push(sha256_file(path)?);
                }
            }
            if path
                .components()
                .any(|component| component.as_os_str() == "packed_records")
                && is_packed_payload_path(path)
            {
                packed_record_file_count += 1;
            }
            Ok(())
        })?;
    }
    if paths.packed_root().exists() {
        for entry in fs::read_dir(paths.packed_root())? {
            let path = entry?.path();
            if !is_packed_index_path(&path) {
                continue;
            }
            for row in read_packed_index_file(&path)? {
                packed_record_row_count += 1;
                record_count += 1;
                if record_hash_sample.len() < SAMPLE_LIMIT {
                    record_hash_sample.push(row.record_sha256);
                }
            }
        }
    }
    Ok(StoreState {
        label: label.to_string(),
        root: paths.cache_root.display().to_string(),
        file_count,
        byte_count,
        index_path: paths.index_path().display().to_string(),
        manifest_path: paths.manifest_path().display().to_string(),
        schema_path: paths.schema_path().display().to_string(),
        cache_record_count: record_count,
        legacy_record_file_count,
        packed_record_row_count,
        packed_record_file_count,
        cache_record_hash_sample: record_hash_sample,
        cache_record_hash_sample_truncated: record_count > SAMPLE_LIMIT,
        disk: disk_snapshot()?,
        hot_root_bytes: path_bytes(&paths.hot_root)?,
        durable_root_bytes: byte_count,
    })
}

fn schema_json() -> Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "Python ME-JEPA-Code forward cache record",
        "type": "object",
        "required": ["schema_version", "cache_key_sha256", "key", "chunk", "instrument_id", "instrument_kind", "output", "source_readback_verified"],
        "properties": {
            "schema_version": {"const": CACHE_SCHEMA_VERSION},
            "cache_key_sha256": {"type": "string", "pattern": "^sha256:[0-9a-f]{64}$"},
            "key": {"type": "object"},
            "output": {"type": "object", "required": ["vector", "vector_len", "vector_sha256"]},
            "source_readback_verified": {"const": true}
        }
    })
}

fn write_json_checked(path: &Path, value: &impl Serialize) -> FsvResult<()> {
    let bytes = serde_json::to_vec_pretty(value)?;
    write_bytes_atomic(path, &bytes)?;
    let readback = fs::read(path)?;
    require(
        &format!(
            "JSON byte readback equality path={} expected_len={} actual_len={} expected_sha256={} actual_sha256={}",
            path.display(),
            bytes.len(),
            readback.len(),
            sha256_prefixed_bytes(&bytes),
            sha256_prefixed_bytes(&readback),
        ),
        readback == bytes,
    )?;
    Ok(())
}

fn write_jsonl_checked(path: &Path, rows: &[Value]) -> FsvResult<()> {
    let mut bytes = Vec::new();
    for row in rows {
        bytes.extend_from_slice(serde_json::to_string(row)?.as_bytes());
        bytes.push(b'\n');
    }
    write_bytes_atomic(path, &bytes)?;
    let readback = fs::read(path)?;
    require(
        &format!(
            "JSONL byte readback equality path={} expected_len={} actual_len={} expected_sha256={} actual_sha256={}",
            path.display(),
            bytes.len(),
            readback.len(),
            sha256_prefixed_bytes(&bytes),
            sha256_prefixed_bytes(&readback),
        ),
        readback == bytes,
    )?;
    Ok(())
}

fn append_jsonl_row_checked<T: Serialize>(path: &Path, row: &T) -> FsvResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let before_rows = count_jsonl_rows(path)?;
    let before_bytes = if path.exists() {
        fs::metadata(path)?.len()
    } else {
        0
    };
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    file.write_all(serde_json::to_string(row)?.as_bytes())?;
    file.write_all(b"\n")?;
    file.flush()?;
    file.sync_all()?;
    drop(file);
    if let Some(parent) = path.parent() {
        File::open(parent)?.sync_all()?;
    }
    require(
        "appended JSONL row increases row count by one",
        count_jsonl_rows(path)? == before_rows + 1,
    )?;
    require(
        "appended JSONL row increases byte count",
        fs::metadata(path)?.len() > before_bytes,
    )?;
    Ok(())
}

fn is_packed_index_path(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.ends_with(".index.jsonl"))
        .unwrap_or(false)
}

fn is_packed_payload_path(path: &Path) -> bool {
    path.extension().and_then(|ext| ext.to_str()) == Some("jsonl") && !is_packed_index_path(path)
}

struct JsonlSink {
    path: PathBuf,
    file: File,
    rows: usize,
    sync_every_rows: usize,
}

impl JsonlSink {
    fn create(path: &Path, sync_every_rows: usize) -> FsvResult<Self> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .mode(0o600)
            .open(path)?;
        Ok(Self {
            path: path.to_path_buf(),
            file,
            rows: 0,
            sync_every_rows,
        })
    }

    fn write_row(&mut self, row: &Value) -> FsvResult<()> {
        self.file
            .write_all(serde_json::to_string(row)?.as_bytes())?;
        self.file.write_all(b"\n")?;
        self.rows += 1;
        if self.sync_every_rows > 0 && self.rows.is_multiple_of(self.sync_every_rows) {
            self.file.sync_all()?;
        }
        Ok(())
    }

    fn finish(self) -> FsvResult<JsonlWriteState> {
        self.file.sync_all()?;
        drop(self.file);
        if let Some(parent) = self.path.parent() {
            File::open(parent)?.sync_all()?;
        }
        let readback_row_count = count_jsonl_rows(&self.path)?;
        require(
            "JSONL sink readback row count matches written row count",
            readback_row_count == self.rows,
        )?;
        Ok(JsonlWriteState {
            path: self.path.display().to_string(),
            row_count: self.rows,
            byte_count: fs::metadata(&self.path)?.len(),
            sha256: sha256_file(&self.path)?,
            readback_row_count,
        })
    }
}

fn count_jsonl_rows(path: &Path) -> FsvResult<usize> {
    if !path.exists() {
        return Ok(0);
    }
    let file = File::open(path)?;
    let mut count = 0usize;
    for line in BufReader::new(file).lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let _: Value = serde_json::from_str(&line)?;
        count += 1;
    }
    Ok(count)
}

fn write_bytes_atomic(path: &Path, bytes: &[u8]) -> FsvResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension(format!(
        "{}.tmp-{}",
        path.extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("json"),
        unix_ms()?
    ));
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(&tmp)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    fs::rename(&tmp, path)?;
    if let Some(parent) = path.parent() {
        File::open(parent)?.sync_all()?;
    }
    Ok(())
}

fn vector_sha256(vector: &[f32]) -> String {
    let mut hasher = Sha256::new();
    for value in vector {
        hasher.update(value.to_le_bytes());
    }
    hex_prefixed(hasher.finalize().as_slice())
}

fn sha256_prefixed_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex_prefixed(hasher.finalize().as_slice())
}

fn key_hash(key: &ForwardCacheKey) -> FsvResult<String> {
    Ok(sha256_prefixed_bytes(&serde_json::to_vec(key)?))
}

fn sha256_file(path: &Path) -> FsvResult<String> {
    Ok(sha256_prefixed_bytes(&fs::read(path)?))
}

fn hex_prefixed(bytes: &[u8]) -> String {
    let mut out = String::from("sha256:");
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

fn is_prefixed_sha256(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn disk_snapshot() -> FsvResult<Value> {
    let output = Command::new("df")
        .args([
            "-B1",
            "--output=source,size,used,avail,pcent,target",
            "/",
            "/mnt/c",
            "/mnt/d",
        ])
        .output()?;
    Ok(json!({
        "command": "df -B1 --output=source,size,used,avail,pcent,target / /mnt/c /mnt/d",
        "exit_code": output.status.code(),
        "stdout": String::from_utf8_lossy(&output.stdout).to_string(),
        "stderr": String::from_utf8_lossy(&output.stderr).to_string(),
    }))
}

fn path_bytes(path: &Path) -> FsvResult<u64> {
    if !path.exists() {
        return Ok(0);
    }
    let mut total = 0u64;
    visit_files(path, &mut |file| {
        total = total.saturating_add(fs::metadata(file)?.len());
        Ok(())
    })?;
    Ok(total)
}

fn visit_files<F>(path: &Path, f: &mut F) -> FsvResult<()>
where
    F: FnMut(&Path) -> FsvResult<()>,
{
    if !path.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(path)? {
        let path = entry?.path();
        if path.is_dir() {
            visit_files(&path, f)?;
        } else {
            f(&path)?;
        }
    }
    Ok(())
}

fn env_path(name: &str, default: &str) -> PathBuf {
    std::env::var(name)
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(default))
}

fn env_usize(name: &str, default: usize) -> FsvResult<usize> {
    match std::env::var(name) {
        Ok(value) => Ok(value.parse()?),
        Err(_) => Ok(default),
    }
}

fn default_cpu_workers() -> usize {
    std::thread::available_parallelism()
        .map(|threads| threads.get())
        .unwrap_or(DEFAULT_CPU_WORKERS_FALLBACK)
}

fn env_usize_optional(name: &str) -> FsvResult<Option<usize>> {
    match std::env::var(name) {
        Ok(value) => Ok(Some(value.parse()?)),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(err) => Err(format!("{name} could not be read as UTF-8: {err}").into()),
    }
}

fn env_u64(name: &str, default: u64) -> FsvResult<u64> {
    match std::env::var(name) {
        Ok(value) => Ok(value.parse()?),
        Err(_) => Ok(default),
    }
}

fn env_u64_optional(name: &str) -> FsvResult<Option<u64>> {
    match std::env::var(name) {
        Ok(value) => Ok(Some(value.parse()?)),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(err) => Err(format!("{name} could not be read as UTF-8: {err}").into()),
    }
}

fn env_embedder_optional(name: &str) -> FsvResult<Option<EmbedderId>> {
    match std::env::var(name) {
        Ok(value) => {
            let trimmed = value.trim();
            require(
                "isolated embedder environment value is non-empty",
                !trimmed.is_empty(),
            )?;
            Ok(Some(EmbedderId::parse(trimmed)?))
        }
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(err) => Err(format!("{name} could not be read as UTF-8: {err}").into()),
    }
}

fn available_bytes(path: &Path) -> FsvResult<u64> {
    let output = Command::new("df")
        .args(["-B1", "--output=avail"])
        .arg(path)
        .output()?;
    require(
        "df available-byte command succeeded",
        output.status.success(),
    )?;
    let stdout = String::from_utf8(output.stdout)?;
    let value = stdout
        .lines()
        .skip(1)
        .find_map(|line| line.trim().parse::<u64>().ok())
        .ok_or_else(|| format!("could not parse df available bytes for {}", path.display()))?;
    Ok(value)
}

fn unix_ms() -> FsvResult<u128> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis())
}

fn utc_now() -> FsvResult<String> {
    let output = Command::new("date")
        .args(["-u", "+%Y-%m-%dT%H:%M:%SZ"])
        .output()?;
    require("date command succeeded", output.status.success())?;
    Ok(String::from_utf8(output.stdout)?.trim().to_string())
}

fn require(label: &str, condition: bool) -> FsvResult<()> {
    if condition {
        Ok(())
    } else {
        Err(format!("python forward cache FSV assertion failed: {label}").into())
    }
}

trait OutputNorm {
    fn vector_l2_norm(&self) -> f64;
}

impl OutputNorm for EmbedderOutput {
    fn vector_l2_norm(&self) -> f64 {
        self.vector
            .iter()
            .map(|value| (*value as f64) * (*value as f64))
            .sum::<f64>()
            .sqrt()
    }
}
