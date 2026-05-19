use std::env;
use std::path::PathBuf;
use std::process::Command;

use context_graph_embeddings::models::CodeModel;
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
            "/zfs/archive/contextgraph/fsv/task-py-data-033-qwen2-code-batch-fsv/qwen2_code_true_batch_fsv.json",
        )
    });
    let telemetry_path = output_path.with_file_name("qwen2_code_true_batch_telemetry.jsonl");
    let model_root = env::var("CONTEXT_GRAPH_MODELS_PATH")
        .or_else(|_| env::var("CONTEXTGRAPH_MODELS_ROOT"))
        .or_else(|_| env::var("EMBEDDING_MODELS_DIR"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/zfs/hot/contextgraph/models"));
    let model_path = model_root.join("code-1536");

    let chunks = synthetic_python_chunks();
    let inputs = chunks
        .iter()
        .map(|chunk| ModelInput::code(&chunk.source, "python"))
        .collect::<Result<Vec<_>, _>>()?;

    let model = CodeModel::new(&model_path, SingleModelConfig::default())?;
    let before_loaded = model.is_initialized();
    let supports_true_batch = model.supports_true_batch();
    let vram_plan_bytes = get_memory_estimate(ModelId::Code);

    model.load().await?;
    let after_load_loaded = model.is_initialized();

    let started = std::time::Instant::now();
    let happy = model.embed_true_batch(&inputs).await?;
    let happy_latency_us = started.elapsed().as_micros() as u64;
    assert_embeddings(&happy, ModelId::Code.dimension())?;

    let empty_batch_error = model
        .embed_true_batch(&[])
        .await
        .expect_err("empty true batch must fail closed");
    let empty_source_error = ModelInput::code("", "python")
        .expect_err("empty source chunk must fail before model submission");
    let unsupported_error = model
        .embed_true_batch(&[ModelInput::Image {
            bytes: vec![0u8; 32],
            format: ImageFormat::Png,
        }])
        .await
        .expect_err("unsupported modality must fail closed");

    let over_limit_text = (0..(model.max_tokens() + 1024))
        .map(|idx| format!("synthetic_overlimit_token_{idx}"))
        .collect::<Vec<_>>()
        .join(" ");
    let over_limit_error = model
        .embed_true_batch(&[ModelInput::code(&over_limit_text, "python")?])
        .await
        .expect_err("over-token-limit source must fail closed");

    let repeat_source = "def repeated(value):\n    return value * 2\n";
    let repeated_inputs = vec![
        ModelInput::code(repeat_source, "python")?,
        ModelInput::code(repeat_source, "python")?,
    ];
    let repeated = model.embed_true_batch(&repeated_inputs).await?;
    assert_embeddings(&repeated, ModelId::Code.dimension())?;
    let repeated_summaries = summarize_embeddings(&repeated);
    let repeated_hashes_equal = repeated_summaries[0]["sha256"] == repeated_summaries[1]["sha256"];

    model.unload().await?;
    let after_unload_loaded = model.is_initialized();

    let happy_summary = summarize_embeddings(&happy);
    let telemetry = json!({
        "event": "qwen2_code_true_batch",
        "model_id": "Code",
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
        "output_dim": ModelId::Code.dimension(),
        "vector_hashes": happy_summary.iter().map(|item| item["sha256"].clone()).collect::<Vec<_>>(),
    });

    let artifact = json!({
        "schema": "contextgraph.task-py-data-033.qwen2-code-true-batch-fsv.v1",
        "source_of_truth": {
            "artifact_path": output_path,
            "telemetry_path": telemetry_path,
            "model_root": model_root,
            "model_path": model_path,
            "verification": "read this JSON and telemetry JSONL after execution"
        },
        "capacity_evidence": {
            "root": df_capacity("/")?,
            "mnt_c": df_capacity("/mnt/c")?,
            "mnt_d": df_capacity("/mnt/d")?,
        },
        "assertions": {
            "supports_true_batch": supports_true_batch,
            "loaded_for_happy_path": after_load_loaded,
            "unloaded_after_edges": !after_unload_loaded,
            "happy_path_returns_8_vectors": happy.len() == 8,
            "happy_path_dimensions_match": happy.iter().all(|embedding| embedding.vector.len() == ModelId::Code.dimension()),
            "empty_batch_fails_closed": format!("{:?}", empty_batch_error).contains("TrueBatchEmpty"),
            "empty_source_fails_before_promotion": format!("{:?}", empty_source_error).contains("EmptyInput"),
            "unsupported_modality_fails_closed": format!("{:?}", unsupported_error).contains("UnsupportedModality"),
            "overlimit_fails_closed": format!("{:?}", over_limit_error).contains("InputTooLong"),
            "repeated_chunk_hashes_equal": repeated_hashes_equal,
        },
        "model": {
            "model_id": "Code",
            "artifact_path": model_path,
            "expected_dim": ModelId::Code.dimension(),
            "max_tokens": model.max_tokens(),
            "vram_plan_bytes": vram_plan_bytes,
            "before_loaded": before_loaded,
            "after_load_loaded": after_load_loaded,
            "after_unload_loaded": after_unload_loaded,
        },
        "happy_path": {
            "chunk_count": chunks.len(),
            "chunks": chunks.iter().map(|chunk| json!({
                "chunk_id": chunk.id,
                "source_byte_start": chunk.start,
                "source_byte_end": chunk.end,
                "source_sha256": sha256_bytes(chunk.source.as_bytes()),
            })).collect::<Vec<_>>(),
            "latency_us": happy_latency_us,
            "embeddings": happy_summary,
        },
        "edge_cases": {
            "empty_batch_error": format!("{:?}", empty_batch_error),
            "empty_source_error": format!("{:?}", empty_source_error),
            "unsupported_modality_error": format!("{:?}", unsupported_error),
            "overlimit_error": format!("{:?}", over_limit_error),
            "repeated_chunk_embeddings": repeated_summaries,
        },
    });

    if artifact["assertions"]
        .as_object()
        .expect("assertions object")
        .values()
        .any(|value| value.as_bool() != Some(true))
    {
        return Err(format!("FSV assertions failed: {}", artifact["assertions"]).into());
    }

    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&output_path, serde_json::to_vec_pretty(&artifact)?)?;
    std::fs::write(
        &telemetry_path,
        format!("{}\n", serde_json::to_string(&telemetry)?),
    )?;
    println!("{}", serde_json::to_string_pretty(&artifact)?);
    Ok(())
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
