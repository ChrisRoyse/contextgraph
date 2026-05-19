use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

use context_graph_embeddings::models::{
    validate_late_interaction_batch_vram_budget, LateInteractionModel, TokenEmbeddings,
    LATE_INTERACTION_DIMENSION,
};
use context_graph_embeddings::{
    get_memory_estimate, EmbeddingModel, ImageFormat, ModelEmbedding, ModelId, ModelInput,
    SingleModelConfig,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

type FsvResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[tokio::main(flavor = "current_thread")]
async fn main() -> FsvResult<()> {
    let output_path = env::args().nth(1).map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(
            "/zfs/archive/contextgraph/fsv/task-py-data-035-colbert-batch-fsv/colbert_true_batch_fsv.json",
        )
    });
    let telemetry_path = output_path.with_file_name("colbert_true_batch_telemetry.jsonl");
    let model_root = env::var("CONTEXT_GRAPH_MODELS_PATH")
        .or_else(|_| env::var("CONTEXTGRAPH_MODELS_ROOT"))
        .or_else(|_| env::var("EMBEDDING_MODELS_DIR"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/zfs/hot/contextgraph/models"));

    let chunks = synthetic_python_chunks();
    let model_run = run_model_case(&model_root, &chunks).await?;

    let malformed_error = TokenEmbeddings::new(
        vec![vec![0.0f32; LATE_INTERACTION_DIMENSION - 1]],
        vec!["synthetic_bad_dim".to_string()],
        vec![true],
    )
    .expect_err("malformed ColBERT token row must fail closed before durable promotion");
    let zero_valid = TokenEmbeddings::new(
        vec![vec![0.0f32; LATE_INTERACTION_DIMENSION]],
        vec!["[PAD]".to_string()],
        vec![false],
    )?;
    let zero_valid_error = zero_valid
        .validate_true_batch_output(ModelId::LateInteraction, 0)
        .expect_err("zero-valid-token ColBERT row must fail closed before durable promotion");

    let assertions = json!({
        "supports_true_batch": model_run["supports_true_batch"].as_bool() == Some(true),
        "happy_path_returns_8_pooled_vectors": model_run["happy_path"]["embedding_count"].as_u64() == Some(8),
        "happy_path_returns_8_token_rows": model_run["happy_path"]["token_row_count"].as_u64() == Some(8),
        "pooled_dimensions_match": all_pooled_dims_match(&model_run),
        "token_dimensions_match": all_token_dims_match(&model_run),
        "token_hashes_present": all_token_hashes_present(&model_run),
        "row_order_preserved": model_run["happy_path"]["row_order_preserved"].as_bool() == Some(true),
        "variable_token_lengths_observed": model_run["happy_path"]["variable_token_lengths_observed"].as_bool() == Some(true),
        "max_batch_guard_fails_closed": model_run["edge_cases"]["max_batch_guard_error"].as_str().is_some_and(|err| err.contains("max_batch_size")),
        "vram_guard_too_small_fails_closed": model_run["edge_cases"]["vram_guard_too_small_error"].as_str().is_some_and(|err| err.contains("InsufficientVram")),
        "empty_batch_fails_closed": model_run["edge_cases"]["empty_batch_error"].as_str().is_some_and(|err| err.contains("TrueBatchEmpty")),
        "empty_source_fails_closed": model_run["edge_cases"]["empty_source_error"].as_str().is_some_and(|err| err.contains("Empty")),
        "unsupported_modality_fails_closed": model_run["edge_cases"]["unsupported_modality_error"].as_str().is_some_and(|err| err.contains("UnsupportedModality")),
        "overlimit_fails_closed": model_run["edge_cases"]["overlimit_error"].as_str().is_some_and(|err| err.contains("InputTooLong")),
        "malformed_output_fails_closed": format!("{malformed_error:?}").contains("InvalidDimension"),
        "zero_valid_output_fails_closed": format!("{zero_valid_error:?}").contains("InferenceValidationFailed"),
    });

    if assertions
        .as_object()
        .expect("assertions object")
        .values()
        .any(|value| value.as_bool() != Some(true))
    {
        return Err(format!("FSV assertions failed: {assertions}").into());
    }

    let telemetry_rows = [model_run["telemetry"].clone()];
    let artifact = json!({
        "schema": "contextgraph.task-py-data-035.colbert-true-batch-fsv.v1",
        "source_of_truth": {
            "artifact_path": output_path,
            "telemetry_path": telemetry_path,
            "model_root": model_root,
            "verification": "read this JSON and telemetry JSONL after execution"
        },
        "trigger_process_outcome": {
            "trigger": "cargo run -p context-graph-embeddings --example colbert_true_batch_fsv -- <artifact_path>",
            "process": "LateInteractionModel::embed_tokens_batch and EmbeddingModel::embed_true_batch",
            "outcome": "durable JSON/JSONL ColBERT true-batch evidence under /zfs/archive/contextgraph/fsv/task-py-data-035-colbert-batch-fsv/"
        },
        "capacity_evidence": {
            "root": df_capacity("/")?,
            "mnt_c": df_capacity("/mnt/c")?,
            "mnt_d": df_capacity("/mnt/d")?,
        },
        "assertions": assertions,
        "model_run": model_run,
        "cross_row_edge_cases": {
            "malformed_output_error": format!("{malformed_error:?}"),
            "zero_valid_output_error": format!("{zero_valid_error:?}"),
        },
    });

    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&output_path, serde_json::to_vec_pretty(&artifact)?)?;
    let telemetry_body = telemetry_rows
        .iter()
        .map(serde_json::to_string)
        .collect::<Result<Vec<_>, _>>()?
        .join("\n");
    std::fs::write(&telemetry_path, format!("{telemetry_body}\n"))?;
    println!("{}", serde_json::to_string_pretty(&artifact)?);
    Ok(())
}

async fn run_model_case(model_root: &Path, chunks: &[SyntheticChunk]) -> FsvResult<Value> {
    let model_path = model_root.join("late-interaction");
    let inputs = chunks
        .iter()
        .map(|chunk| ModelInput::text(&chunk.source))
        .collect::<Result<Vec<_>, _>>()?;
    let texts = chunks
        .iter()
        .map(|chunk| chunk.source.clone())
        .collect::<Vec<_>>();
    let model = LateInteractionModel::new(&model_path, SingleModelConfig::default())?;
    let before_loaded = model.is_initialized();
    let supports_true_batch = model.supports_true_batch();
    let vram_plan_bytes = get_memory_estimate(ModelId::LateInteraction);
    let vram_guard_error = validate_late_interaction_batch_vram_budget(
        0,
        vram_plan_bytes
            .checked_sub(1)
            .expect("vram plan must be positive"),
    )
    .expect_err("too-small ColBERT VRAM budget must fail before forward");

    model.load().await?;
    let after_load_loaded = model.is_initialized();

    let started = std::time::Instant::now();
    let token_rows = model.embed_tokens_batch(&texts).await?;
    let pooled_embeddings = model.embed_true_batch(&inputs).await?;
    let happy_latency_us = started.elapsed().as_micros() as u64;
    assert_embeddings(&pooled_embeddings)?;

    let token_summaries = token_rows
        .iter()
        .map(summarize_token_embeddings)
        .collect::<Vec<_>>();
    let pooled_summaries = summarize_embeddings(&pooled_embeddings, chunks);
    let token_counts = token_summaries
        .iter()
        .map(|item| item["valid_token_count"].clone())
        .collect::<Vec<_>>();
    let max_seq_len = token_summaries
        .iter()
        .filter_map(|item| item["total_token_count"].as_u64())
        .max()
        .unwrap_or(0);
    let padding_tokens = token_summaries
        .iter()
        .filter_map(|item| item["padding_token_count"].as_u64())
        .sum::<u64>();
    let row_order_preserved = pooled_summaries
        .iter()
        .zip(chunks.iter())
        .all(|(summary, chunk)| summary["chunk_id"].as_str() == Some(chunk.id));
    let variable_token_lengths_observed = token_summaries
        .windows(2)
        .any(|pair| pair[0]["valid_token_count"] != pair[1]["valid_token_count"]);

    let empty_batch_error = model
        .embed_true_batch(&[])
        .await
        .expect_err("empty true batch must fail closed");
    let empty_source_error =
        ModelInput::text("").expect_err("empty source chunk must fail before model submission");
    let unsupported_error = model
        .embed_true_batch(&[ModelInput::Image {
            bytes: vec![0u8; 32],
            format: ImageFormat::Png,
        }])
        .await
        .expect_err("unsupported modality must fail closed");
    let over_limit_text = (0..(model.max_tokens() + 64))
        .map(|idx| format!("synthetic_overlimit_token_{idx}"))
        .collect::<Vec<_>>()
        .join(" ");
    let over_limit_error = model
        .embed_true_batch(&[ModelInput::text(&over_limit_text)?])
        .await
        .expect_err("over-token-limit source must fail closed");

    model.unload().await?;
    let after_unload_loaded = model.is_initialized();

    let guard_config = SingleModelConfig {
        max_batch_size: 1,
        ..SingleModelConfig::default()
    };
    let guard_model = LateInteractionModel::new(&model_path, guard_config)?;
    guard_model.load().await?;
    let guard_error = guard_model
        .embed_true_batch(&inputs[..2])
        .await
        .expect_err("max_batch_size guard must fail before CUDA forward");
    guard_model.unload().await?;

    let telemetry = json!({
        "event": "colbert_true_batch",
        "model_id": "LateInteraction",
        "model_dir": "late-interaction",
        "batch_size": inputs.len(),
        "chunk_ids": chunks.iter().map(|chunk| chunk.id).collect::<Vec<_>>(),
        "source_byte_bounds": chunks.iter().map(|chunk| json!({
            "chunk_id": chunk.id,
            "start": chunk.start,
            "end": chunk.end,
            "sha256": sha256_bytes(chunk.source.as_bytes()),
        })).collect::<Vec<_>>(),
        "vram_plan_bytes": vram_plan_bytes,
        "latency_us": happy_latency_us,
        "max_seq_len": max_seq_len,
        "valid_token_counts": token_counts,
        "padding_tokens": padding_tokens,
        "token_dim": LATE_INTERACTION_DIMENSION,
        "pooled_vector_hashes": pooled_summaries.iter().map(|item| item["sha256"].clone()).collect::<Vec<_>>(),
        "token_hashes": token_summaries.iter().map(|item| item["token_sha256"].clone()).collect::<Vec<_>>(),
    });

    Ok(json!({
        "model_id": "LateInteraction",
        "model_dir": "late-interaction",
        "model_path": model_path,
        "supports_true_batch": supports_true_batch,
        "before_loaded": before_loaded,
        "after_load_loaded": after_load_loaded,
        "after_unload_loaded": after_unload_loaded,
        "expected_dimension": LATE_INTERACTION_DIMENSION,
        "max_tokens": model.max_tokens(),
        "vram_plan_bytes": vram_plan_bytes,
        "happy_path": {
            "chunk_count": chunks.len(),
            "embedding_count": pooled_embeddings.len(),
            "token_row_count": token_rows.len(),
            "chunks": chunks.iter().map(|chunk| json!({
                "chunk_id": chunk.id,
                "source_byte_start": chunk.start,
                "source_byte_end": chunk.end,
                "source_sha256": sha256_bytes(chunk.source.as_bytes()),
            })).collect::<Vec<_>>(),
            "latency_us": happy_latency_us,
            "row_order_preserved": row_order_preserved,
            "variable_token_lengths_observed": variable_token_lengths_observed,
            "pooled_embeddings": pooled_summaries,
            "token_embeddings": token_summaries,
        },
        "edge_cases": {
            "empty_batch_error": format!("{:?}", empty_batch_error),
            "empty_source_error": format!("{:?}", empty_source_error),
            "unsupported_modality_error": format!("{:?}", unsupported_error),
            "overlimit_error": format!("{:?}", over_limit_error),
            "max_batch_guard_error": format!("{:?}", guard_error),
            "vram_guard_too_small_error": format!("{:?}", vram_guard_error),
        },
        "telemetry": telemetry,
    }))
}

fn summarize_embeddings(embeddings: &[ModelEmbedding], chunks: &[SyntheticChunk]) -> Vec<Value> {
    embeddings
        .iter()
        .enumerate()
        .map(|(idx, embedding)| {
            json!({
                "chunk_id": chunks[idx].id,
                "model_id": format!("{:?}", embedding.model_id),
                "dimension": embedding.vector.len(),
                "l2_norm": l2_norm(&embedding.vector),
                "sha256": sha256_f32(&embedding.vector),
                "latency_us": embedding.latency_us,
            })
        })
        .collect()
}

fn summarize_token_embeddings(tokens: &TokenEmbeddings) -> Value {
    let token_dim = tokens.vectors.first().map_or(0, Vec::len);
    let valid_token_count = tokens.valid_token_count();
    json!({
        "total_token_count": tokens.vectors.len(),
        "valid_token_count": valid_token_count,
        "padding_token_count": tokens.vectors.len().saturating_sub(valid_token_count),
        "token_dim": token_dim,
        "mask_true_count": tokens.mask.iter().filter(|valid| **valid).count(),
        "first_tokens": tokens.tokens.iter().take(6).collect::<Vec<_>>(),
        "token_sha256": sha256_token_embeddings(tokens),
    })
}

fn assert_embeddings(embeddings: &[ModelEmbedding]) -> FsvResult<()> {
    for embedding in embeddings {
        embedding.validate()?;
        if embedding.model_id != ModelId::LateInteraction {
            return Err(format!("unexpected model id {:?}", embedding.model_id).into());
        }
        if embedding.vector.len() != LATE_INTERACTION_DIMENSION {
            return Err(format!(
                "unexpected vector dim {}; expected {}",
                embedding.vector.len(),
                LATE_INTERACTION_DIMENSION
            )
            .into());
        }
    }
    Ok(())
}

fn all_pooled_dims_match(run: &Value) -> bool {
    run["happy_path"]["pooled_embeddings"]
        .as_array()
        .is_some_and(|items| {
            items
                .iter()
                .all(|item| item["dimension"].as_u64() == Some(LATE_INTERACTION_DIMENSION as u64))
        })
}

fn all_token_dims_match(run: &Value) -> bool {
    run["happy_path"]["token_embeddings"]
        .as_array()
        .is_some_and(|items| {
            items
                .iter()
                .all(|item| item["token_dim"].as_u64() == Some(LATE_INTERACTION_DIMENSION as u64))
        })
}

fn all_token_hashes_present(run: &Value) -> bool {
    run["happy_path"]["token_embeddings"]
        .as_array()
        .is_some_and(|items| {
            items.iter().all(|item| {
                item["token_sha256"]
                    .as_str()
                    .is_some_and(|value| value.len() == 64)
            })
        })
}

fn l2_norm(vector: &[f32]) -> f32 {
    vector.iter().map(|v| v * v).sum::<f32>().sqrt()
}

fn sha256_f32(vector: &[f32]) -> String {
    let mut hasher = Sha256::new();
    for value in vector {
        hasher.update(value.to_le_bytes());
    }
    format!("{:x}", hasher.finalize())
}

fn sha256_token_embeddings(tokens: &TokenEmbeddings) -> String {
    let mut hasher = Sha256::new();
    for (idx, vector) in tokens.vectors.iter().enumerate() {
        hasher.update((idx as u64).to_le_bytes());
        hasher.update(tokens.tokens[idx].as_bytes());
        hasher.update([u8::from(tokens.mask[idx])]);
        for value in vector {
            hasher.update(value.to_le_bytes());
        }
    }
    format!("{:x}", hasher.finalize())
}

fn sha256_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn df_capacity(path: &str) -> FsvResult<Value> {
    let output = Command::new("df").args(["-Pk", path]).output()?;
    if !output.status.success() {
        return Err(format!("df -Pk {path} failed with status {}", output.status).into());
    }
    let stdout = String::from_utf8(output.stdout)?;
    let line = stdout
        .lines()
        .nth(1)
        .ok_or_else(|| format!("df -Pk {path} missing data line"))?;
    let cols = line.split_whitespace().collect::<Vec<_>>();
    if cols.len() < 6 {
        return Err(format!("df -Pk {path} returned malformed line: {line}").into());
    }
    Ok(json!({
        "filesystem": cols[0],
        "blocks_1k": cols[1].parse::<u64>()?,
        "used_1k": cols[2].parse::<u64>()?,
        "available_1k": cols[3].parse::<u64>()?,
        "capacity": cols[4],
        "mount": cols[5],
    }))
}

#[derive(Clone)]
struct SyntheticChunk {
    id: &'static str,
    source: String,
    start: usize,
    end: usize,
}

fn synthetic_python_chunks() -> Vec<SyntheticChunk> {
    let sources = vec![
        (
            "py_colbert_chunk_001",
            "def add_two(value):\n    return value + 2\n",
        ),
        (
            "py_colbert_chunk_002",
            "def clamp(value, lower, upper):\n    return max(lower, min(value, upper))\n",
        ),
        (
            "py_colbert_chunk_003",
            "def normalize(items):\n    total = sum(items)\n    return [item / total for item in items]\n",
        ),
        (
            "py_colbert_chunk_004",
            "class Ledger:\n    def __init__(self):\n        self.rows = []\n    def append(self, row):\n        self.rows.append(row)\n        return len(self.rows)\n",
        ),
        (
            "py_colbert_chunk_005",
            "async def fetch(session, url):\n    response = await session.get(url)\n    return await response.json()\n",
        ),
        (
            "py_colbert_chunk_006",
            "def retry_once(fn, arg):\n    try:\n        return fn(arg)\n    except ValueError:\n        return fn(arg)\n",
        ),
        (
            "py_colbert_chunk_007",
            "def windowed(values, size):\n    return [values[idx:idx + size] for idx in range(0, len(values), size)]\n",
        ),
        (
            "py_colbert_chunk_008",
            "def permissions(user, resource):\n    if user.is_admin:\n        return True\n    return resource.owner_id == user.id\n",
        ),
    ];

    let mut offset = 0usize;
    sources
        .into_iter()
        .map(|(id, source)| {
            let start = offset;
            let end = start + source.len();
            offset = end + 1;
            SyntheticChunk {
                id,
                source: source.to_string(),
                start,
                end,
            }
        })
        .collect()
}
