use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

use context_graph_embeddings::models::{
    validate_true_batch_vram_budget, SparseModel, SparseVector, SPARSE_PROJECTED_DIMENSION,
    SPARSE_VOCAB_SIZE,
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
            "/mnt/d/contextgraph/fsv/task-py-data-034-splade-batch-fsv/splade_true_batch_fsv.json",
        )
    });
    let telemetry_path = output_path.with_file_name("splade_true_batch_telemetry.jsonl");
    let model_root = env::var("CONTEXT_GRAPH_MODELS_PATH")
        .or_else(|_| env::var("CONTEXTGRAPH_MODELS_ROOT"))
        .or_else(|_| env::var("EMBEDDING_MODELS_DIR"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/mnt/d/cabdru/contextgraph/models"));

    let chunks = synthetic_python_chunks();
    let sparse_run = run_model_case(ModelId::Sparse, &model_root, "sparse", &chunks).await?;
    let splade_run = run_model_case(ModelId::Splade, &model_root, "splade-v3", &chunks).await?;
    let all_zero_sparse = SparseVector::default();
    let all_zero_sparse_error = all_zero_sparse
        .validate_true_batch_output(ModelId::Sparse, 0)
        .expect_err("all-zero sparse output must fail closed before projection");
    let all_zero_sparse_error_text = format!("{all_zero_sparse_error:?}");

    let assertions = json!({
        "sparse_supports_true_batch": sparse_run["supports_true_batch"].as_bool() == Some(true),
        "splade_supports_true_batch": splade_run["supports_true_batch"].as_bool() == Some(true),
        "sparse_happy_path_returns_8_vectors": sparse_run["happy_path"]["embedding_count"].as_u64() == Some(8),
        "splade_happy_path_returns_8_vectors": splade_run["happy_path"]["embedding_count"].as_u64() == Some(8),
        "sparse_dimensions_match": all_dims_match(&sparse_run),
        "splade_dimensions_match": all_dims_match(&splade_run),
        "sparse_outputs_have_nonzero_terms": all_nnz_positive(&sparse_run),
        "splade_outputs_have_nonzero_terms": all_nnz_positive(&splade_run),
        "sparse_guard_too_small_fails_closed": sparse_run["edge_cases"]["max_batch_guard_error"].as_str().is_some_and(|err| err.contains("max_batch_size")),
        "splade_guard_too_small_fails_closed": splade_run["edge_cases"]["max_batch_guard_error"].as_str().is_some_and(|err| err.contains("max_batch_size")),
        "sparse_vram_guard_too_small_fails_closed": sparse_run["edge_cases"]["vram_guard_too_small_error"].as_str().is_some_and(|err| err.contains("InsufficientVram")),
        "splade_vram_guard_too_small_fails_closed": splade_run["edge_cases"]["vram_guard_too_small_error"].as_str().is_some_and(|err| err.contains("InsufficientVram")),
        "sparse_empty_batch_fails_closed": sparse_run["edge_cases"]["empty_batch_error"].as_str().is_some_and(|err| err.contains("TrueBatchEmpty")),
        "splade_empty_batch_fails_closed": splade_run["edge_cases"]["empty_batch_error"].as_str().is_some_and(|err| err.contains("TrueBatchEmpty")),
        "sparse_overlimit_fails_closed": sparse_run["edge_cases"]["overlimit_error"].as_str().is_some_and(|err| err.contains("InputTooLong")),
        "splade_overlimit_fails_closed": splade_run["edge_cases"]["overlimit_error"].as_str().is_some_and(|err| err.contains("InputTooLong")),
        "sparse_unsupported_modality_fails_closed": sparse_run["edge_cases"]["unsupported_modality_error"].as_str().is_some_and(|err| err.contains("UnsupportedModality")),
        "splade_unsupported_modality_fails_closed": splade_run["edge_cases"]["unsupported_modality_error"].as_str().is_some_and(|err| err.contains("UnsupportedModality")),
        "all_zero_sparse_output_fails_closed": all_zero_sparse.nnz() == 0 && all_zero_sparse.dimension == SPARSE_VOCAB_SIZE && all_zero_sparse_error_text.contains("InferenceValidationFailed"),
    });

    if assertions
        .as_object()
        .expect("assertions object")
        .values()
        .any(|value| value.as_bool() != Some(true))
    {
        return Err(format!("FSV assertions failed: {assertions}").into());
    }

    let telemetry_rows = [
        sparse_run["telemetry"].clone(),
        splade_run["telemetry"].clone(),
    ];
    let artifact = json!({
        "schema": "contextgraph.task-py-data-034.splade-true-batch-fsv.v1",
        "source_of_truth": {
            "artifact_path": output_path,
            "telemetry_path": telemetry_path,
            "model_root": model_root,
            "verification": "read this JSON and telemetry JSONL after execution"
        },
        "capacity_evidence": {
            "root": df_capacity("/")?,
            "mnt_c": df_capacity("/mnt/c")?,
            "mnt_d": df_capacity("/mnt/d")?,
        },
        "assertions": assertions,
        "model_runs": [sparse_run, splade_run],
        "cross_model_edge_cases": {
            "all_zero_sparse_output_error": all_zero_sparse_error_text,
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

async fn run_model_case(
    model_id: ModelId,
    model_root: &Path,
    model_dir: &str,
    chunks: &[SyntheticChunk],
) -> FsvResult<Value> {
    let model_path = model_root.join(model_dir);
    let inputs = chunks
        .iter()
        .map(|chunk| ModelInput::text(&chunk.source))
        .collect::<Result<Vec<_>, _>>()?;
    let model = create_sparse_model(model_id, &model_path, SingleModelConfig::default())?;
    let before_loaded = model.is_initialized();
    let supports_true_batch = model.supports_true_batch();
    let vram_plan_bytes = get_memory_estimate(model_id);
    let vram_guard_error = validate_true_batch_vram_budget(
        model_id,
        0,
        vram_plan_bytes
            .checked_sub(1)
            .expect("vram plan must be positive"),
    )
    .expect_err("too-small SPLADE VRAM budget must fail before forward");

    model.load().await?;
    let after_load_loaded = model.is_initialized();

    let started = std::time::Instant::now();
    let dual = model.embed_dual_batch(&inputs).await?;
    let happy_latency_us = started.elapsed().as_micros() as u64;
    let dense_embeddings = dual
        .iter()
        .map(|item| item.dense.clone())
        .collect::<Vec<_>>();
    assert_embeddings(&dense_embeddings, SPARSE_PROJECTED_DIMENSION)?;

    let sparse_summaries = dual
        .iter()
        .map(|item| summarize_sparse(&item.sparse))
        .collect::<Vec<_>>();
    let dense_summaries = summarize_embeddings(&dense_embeddings);

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
    let guard_model = create_sparse_model(model_id, &model_path, guard_config)?;
    guard_model.load().await?;
    let guard_error = guard_model
        .embed_true_batch(&inputs[..2])
        .await
        .expect_err("max_batch_size guard must fail before CUDA forward");
    guard_model.unload().await?;

    let telemetry = json!({
        "event": "splade_true_batch",
        "model_id": format!("{model_id:?}"),
        "model_dir": model_dir,
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
        "projected_output_dim": SPARSE_PROJECTED_DIMENSION,
        "sparse_dim": SPARSE_VOCAB_SIZE,
        "nonzero_counts": sparse_summaries.iter().map(|item| item["nnz"].clone()).collect::<Vec<_>>(),
        "sparse_densities": sparse_summaries.iter().map(|item| item["density"].clone()).collect::<Vec<_>>(),
        "vector_hashes": dense_summaries.iter().map(|item| item["sha256"].clone()).collect::<Vec<_>>(),
        "sparse_hashes": sparse_summaries.iter().map(|item| item["sparse_sha256"].clone()).collect::<Vec<_>>(),
    });

    Ok(json!({
        "model_id": format!("{model_id:?}"),
        "model_dir": model_dir,
        "model_path": model_path,
        "supports_true_batch": supports_true_batch,
        "before_loaded": before_loaded,
        "after_load_loaded": after_load_loaded,
        "after_unload_loaded": after_unload_loaded,
        "expected_projected_dim": SPARSE_PROJECTED_DIMENSION,
        "expected_sparse_dim": SPARSE_VOCAB_SIZE,
        "max_tokens": model.max_tokens(),
        "vram_plan_bytes": vram_plan_bytes,
        "happy_path": {
            "chunk_count": chunks.len(),
            "embedding_count": dense_embeddings.len(),
            "chunks": chunks.iter().map(|chunk| json!({
                "chunk_id": chunk.id,
                "source_byte_start": chunk.start,
                "source_byte_end": chunk.end,
                "source_sha256": sha256_bytes(chunk.source.as_bytes()),
            })).collect::<Vec<_>>(),
            "latency_us": happy_latency_us,
            "dense_embeddings": dense_summaries,
            "sparse_embeddings": sparse_summaries,
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

fn create_sparse_model(
    model_id: ModelId,
    model_path: &Path,
    config: SingleModelConfig,
) -> FsvResult<SparseModel> {
    match model_id {
        ModelId::Sparse => Ok(SparseModel::new(model_path, config)?),
        ModelId::Splade => Ok(SparseModel::new_splade(model_path, config)?),
        _ => Err(format!("unsupported sparse FSV model_id: {model_id:?}").into()),
    }
}

#[derive(Clone)]
struct SyntheticChunk {
    id: &'static str,
    source: String,
    start: usize,
    end: usize,
}

fn synthetic_python_chunks() -> Vec<SyntheticChunk> {
    let sources = [
        ("synthetic_py_chunk_001", "def add(a, b):\n    return a + b\n"),
        (
            "synthetic_py_chunk_002",
            "def guard(value):\n    if value is None:\n        raise ValueError('missing')\n    return value\n",
        ),
        (
            "synthetic_py_chunk_003",
            "class Counter:\n    def __init__(self):\n        self.n = 0\n    def inc(self):\n        self.n += 1\n",
        ),
        (
            "synthetic_py_chunk_004",
            "def normalize(items):\n    return [item.strip().lower() for item in items]\n",
        ),
        (
            "synthetic_py_chunk_005",
            "async def fetch_one(client, url):\n    response = await client.get(url)\n    return response.status_code\n",
        ),
        (
            "synthetic_py_chunk_006",
            "def safe_divide(a, b):\n    if b == 0:\n        return None\n    return a / b\n",
        ),
        (
            "synthetic_py_chunk_007",
            "def emit_witness(chain, event):\n    chain.append({'kind': 'test', 'event': event})\n    return chain\n",
        ),
        (
            "synthetic_py_chunk_008",
            "def patch_summary(paths):\n    return {'changed': len(paths), 'paths': sorted(paths)}\n",
        ),
    ];

    let mut cursor = 0usize;
    sources
        .into_iter()
        .map(|(id, source)| {
            let start = cursor;
            cursor += source.len();
            SyntheticChunk {
                id,
                source: source.to_string(),
                start,
                end: cursor,
            }
        })
        .collect()
}

fn assert_embeddings(embeddings: &[ModelEmbedding], expected_dim: usize) -> FsvResult<()> {
    for embedding in embeddings {
        embedding.validate()?;
        if embedding.vector.len() != expected_dim {
            return Err(format!(
                "dimension mismatch for {:?}: expected {}, got {}",
                embedding.model_id,
                expected_dim,
                embedding.vector.len()
            )
            .into());
        }
        if !embedding.is_projected {
            return Err(format!(
                "{:?} embedding was not marked projected",
                embedding.model_id
            )
            .into());
        }
    }
    Ok(())
}

fn summarize_embeddings(embeddings: &[ModelEmbedding]) -> Vec<Value> {
    embeddings
        .iter()
        .map(|embedding| {
            json!({
                "model_id": format!("{:?}", embedding.model_id),
                "dimension": embedding.vector.len(),
                "latency_us": embedding.latency_us,
                "l2_norm": l2_norm(&embedding.vector),
                "sha256": vector_sha256(&embedding.vector),
                "first4": embedding.vector.iter().take(4).copied().collect::<Vec<_>>(),
            })
        })
        .collect()
}

fn summarize_sparse(sparse: &SparseVector) -> Value {
    json!({
        "dimension": sparse.dimension,
        "nnz": sparse.nnz(),
        "density": sparse.nnz() as f32 / sparse.dimension as f32,
        "sparsity": sparse.sparsity(),
        "sparse_sha256": sparse_sha256(sparse),
        "first8_indices": sparse.indices.iter().take(8).copied().collect::<Vec<_>>(),
        "first8_weights": sparse.weights.iter().take(8).copied().collect::<Vec<_>>(),
    })
}

fn all_dims_match(run: &Value) -> bool {
    run["happy_path"]["dense_embeddings"]
        .as_array()
        .is_some_and(|items| {
            items
                .iter()
                .all(|item| item["dimension"].as_u64() == Some(SPARSE_PROJECTED_DIMENSION as u64))
        })
}

fn all_nnz_positive(run: &Value) -> bool {
    run["happy_path"]["sparse_embeddings"]
        .as_array()
        .is_some_and(|items| {
            items
                .iter()
                .all(|item| item["nnz"].as_u64().is_some_and(|n| n > 0))
        })
}

fn l2_norm(vector: &[f32]) -> f32 {
    vector.iter().map(|value| value * value).sum::<f32>().sqrt()
}

fn vector_sha256(vector: &[f32]) -> String {
    let mut hasher = Sha256::new();
    for value in vector {
        hasher.update(value.to_le_bytes());
    }
    format!("{:x}", hasher.finalize())
}

fn sparse_sha256(sparse: &SparseVector) -> String {
    let mut hasher = Sha256::new();
    for (idx, value) in sparse.indices.iter().zip(sparse.weights.iter()) {
        hasher.update(idx.to_le_bytes());
        hasher.update(value.to_le_bytes());
    }
    format!("{:x}", hasher.finalize())
}

fn sha256_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn df_capacity(path: &str) -> FsvResult<Value> {
    let output = Command::new("df").arg("-B1").arg(path).output()?;
    if !output.status.success() {
        return Err(format!("df -B1 {path} failed with status {}", output.status).into());
    }
    parse_df(path, &String::from_utf8(output.stdout)?)
}

fn parse_df(path: &str, stdout: &str) -> FsvResult<Value> {
    let line = stdout
        .lines()
        .nth(1)
        .ok_or_else(|| format!("df -B1 {path} did not return a data row"))?;
    let columns = line.split_whitespace().collect::<Vec<_>>();
    if columns.len() < 6 {
        return Err(format!("df -B1 {path} returned malformed row: {line}").into());
    }
    Ok(json!({
        "path": path,
        "filesystem": columns[0],
        "bytes_total": columns[1].parse::<u64>()?,
        "bytes_used": columns[2].parse::<u64>()?,
        "bytes_available": columns[3].parse::<u64>()?,
        "used_percent": columns[4],
        "mounted_on": columns[5],
    }))
}
