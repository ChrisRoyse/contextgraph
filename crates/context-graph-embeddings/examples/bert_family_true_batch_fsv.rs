use std::env;
use std::path::{Path, PathBuf};

use context_graph_embeddings::models::{
    BgeM3DenseModel, ContextualModel, GraphModel, SemanticModel,
};
use context_graph_embeddings::{
    EmbeddingModel, ImageFormat, ModelEmbedding, ModelInput, SingleModelConfig,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

type FsvResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[tokio::main(flavor = "current_thread")]
async fn main() -> FsvResult<()> {
    let output_path = env::args().nth(1).map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(
            "/zfs/archive/contextgraph/fsv/task-py-data-032-bert-family-batch-fsv/bert_family_true_batch_fsv.json",
        )
    });
    let model_root = env::var("CONTEXT_GRAPH_MODELS_PATH")
        .or_else(|_| env::var("CONTEXTGRAPH_MODELS_ROOT"))
        .or_else(|_| env::var("EMBEDDING_MODELS_DIR"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/zfs/hot/contextgraph/models"));

    let mut models = Vec::new();

    let semantic = SemanticModel::new(&model_root.join("semantic"), SingleModelConfig::default())?;
    models.push(
        run_model_case(
            "E1 SemanticModel",
            &model_root.join("semantic"),
            &semantic,
            1024,
        )
        .await?,
    );

    let graph = GraphModel::new(&model_root.join("semantic"), SingleModelConfig::default())?;
    models.push(run_model_case("E8 GraphModel", &model_root.join("semantic"), &graph, 1024).await?);

    let contextual = ContextualModel::new(
        &model_root.join("contextual-e5-base"),
        SingleModelConfig::default(),
    )?;
    models.push(
        run_model_case(
            "E10 ContextualModel",
            &model_root.join("contextual-e5-base"),
            &contextual,
            768,
        )
        .await?,
    );

    let bge = BgeM3DenseModel::new(
        &model_root.join("bge-m3-dense"),
        SingleModelConfig::default(),
    )?;
    models.push(
        run_model_case(
            "E14 BgeM3DenseModel",
            &model_root.join("bge-m3-dense"),
            &bge,
            1024,
        )
        .await?,
    );

    let artifact = json!({
        "schema": "contextgraph.task-py-data-032.bert-family-true-batch-fsv.v1",
        "source_of_truth": {
            "artifact_path": output_path,
            "model_root": model_root,
            "verification": "read this JSON after execution and compare model case states"
        },
        "assertions": {
            "all_models_support_true_batch": models.iter().all(|m| m["supports_true_batch"].as_bool() == Some(true)),
            "all_models_loaded_for_happy_path": models.iter().all(|m| m["after_load_loaded"].as_bool() == Some(true)),
            "all_models_unloaded_after_edges": models.iter().all(|m| m["after_unload_loaded"].as_bool() == Some(false)),
            "all_happy_paths_return_two_vectors": models.iter().all(|m| m["happy_path"]["output_count"].as_u64() == Some(2)),
            "all_singleton_paths_return_one_vector": models.iter().all(|m| m["edge_singleton"]["output_count"].as_u64() == Some(1)),
            "all_empty_edges_fail_closed": models.iter().all(|m| m["edge_empty_batch"]["error"].as_str().is_some_and(|s| s.contains("TrueBatchEmpty"))),
            "all_unsupported_edges_fail_closed": models.iter().all(|m| m["edge_unsupported_modality"]["error"].as_str().is_some_and(|s| s.contains("UnsupportedModality"))),
            "all_overlimit_edges_fail_closed": models.iter().all(|m| m["edge_over_token_limit"]["error"].as_str().is_some_and(|s| s.contains("InputTooLong"))),
        },
        "models": models,
    });

    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&output_path, serde_json::to_vec_pretty(&artifact)?)?;
    println!("{}", serde_json::to_string_pretty(&artifact)?);
    Ok(())
}

async fn run_model_case<M>(
    label: &str,
    artifact_path: &Path,
    model: &M,
    expected_dim: usize,
) -> FsvResult<Value>
where
    M: EmbeddingModel,
{
    let before_loaded = model.is_initialized();
    let supports_true_batch = model.supports_true_batch();

    model.load().await?;
    let after_load_loaded = model.is_initialized();

    let happy_inputs = vec![
        ModelInput::text(format!("{label} happy path code reality claim exists"))?,
        ModelInput::text(format!("{label} predicts tests pass in durable reality"))?,
    ];
    let happy = model.embed_true_batch(&happy_inputs).await?;
    assert_embeddings(&happy, expected_dim)?;

    let singleton_inputs = vec![ModelInput::text(format!(
        "{label} singleton true-batch boundary"
    ))?];
    let singleton = model.embed_true_batch(&singleton_inputs).await?;
    assert_embeddings(&singleton, expected_dim)?;

    let empty_error = model
        .embed_true_batch(&[])
        .await
        .expect_err("empty true batch must fail closed");

    let unsupported = ModelInput::Image {
        bytes: vec![0u8; 32],
        format: ImageFormat::Png,
    };
    let unsupported_error = model
        .embed_true_batch(&[unsupported])
        .await
        .expect_err("unsupported modality must fail closed");

    let over_limit_text = (0..(model.max_tokens() + 64))
        .map(|idx| format!("synthetic_overlimit_token_{idx}"))
        .collect::<Vec<_>>()
        .join(" ");
    let over_limit_error = model
        .embed_true_batch(&[ModelInput::text(over_limit_text)?])
        .await
        .expect_err("over-token-limit batch must fail closed");

    model.unload().await?;
    let after_unload_loaded = model.is_initialized();

    Ok(json!({
        "label": label,
        "model_id": format!("{:?}", model.model_id()),
        "artifact_path": artifact_path,
        "expected_dim": expected_dim,
        "before_loaded": before_loaded,
        "supports_true_batch": supports_true_batch,
        "after_load_loaded": after_load_loaded,
        "after_unload_loaded": after_unload_loaded,
        "happy_path": summarize_embeddings(&happy),
        "edge_singleton": summarize_embeddings(&singleton),
        "edge_empty_batch": {
            "error": format!("{:?}", empty_error),
        },
        "edge_unsupported_modality": {
            "error": format!("{:?}", unsupported_error),
        },
        "edge_over_token_limit": {
            "error": format!("{:?}", over_limit_error),
        },
    }))
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

fn summarize_embeddings(embeddings: &[ModelEmbedding]) -> Value {
    json!({
        "output_count": embeddings.len(),
        "vectors": embeddings.iter().map(|embedding| {
            json!({
                "model_id": format!("{:?}", embedding.model_id),
                "dimension": embedding.vector.len(),
                "latency_us": embedding.latency_us,
                "l2_norm": l2_norm(&embedding.vector),
                "sha256": vector_sha256(&embedding.vector),
                "first4": embedding.vector.iter().take(4).copied().collect::<Vec<_>>(),
            })
        }).collect::<Vec<_>>(),
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
