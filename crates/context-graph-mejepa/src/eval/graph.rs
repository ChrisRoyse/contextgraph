use super::error::{EvalError, EvalErrorCode};
use crate::types::TaskId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PatchEmbedding {
    pub task_id: TaskId,
    pub vector: Vec<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PatchGraphEdge {
    pub left: TaskId,
    pub right: TaskId,
    pub cosine_similarity: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PatchSimilarityGraph {
    pub node_count: usize,
    pub edge_count: usize,
    pub threshold: f32,
    pub top_k: usize,
    pub edges: Vec<PatchGraphEdge>,
}

pub fn build_patch_similarity_graph(
    embeddings: &[PatchEmbedding],
    threshold: f32,
    top_k: usize,
) -> Result<PatchSimilarityGraph, EvalError> {
    if embeddings.is_empty() {
        return Err(EvalError::new(
            EvalErrorCode::InvalidInput,
            "patch graph requires at least one embedding",
        ));
    }
    if !threshold.is_finite() || !(-1.0..=1.0).contains(&threshold) {
        return Err(EvalError::new(
            EvalErrorCode::InvalidConfig,
            format!("threshold must be in [-1,1]; got {threshold}"),
        ));
    }
    if top_k == 0 {
        return Err(EvalError::new(
            EvalErrorCode::InvalidConfig,
            "top_k must be greater than zero",
        ));
    }
    let dim = embeddings[0].vector.len();
    if dim == 0 {
        return Err(EvalError::new(
            EvalErrorCode::InvalidInput,
            "patch embeddings must be non-empty",
        ));
    }
    for embedding in embeddings {
        embedding
            .task_id
            .validate("patch_graph.task_id")
            .map_err(EvalError::from)?;
        if embedding.vector.len() != dim {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                "all patch embeddings must have identical dimensions",
            ));
        }
        for value in &embedding.vector {
            if !value.is_finite() {
                return Err(EvalError::new(
                    EvalErrorCode::InvalidInput,
                    "patch embedding contains non-finite value",
                ));
            }
        }
    }

    let mut edges = Vec::new();
    for i in 0..embeddings.len() {
        let mut scored = Vec::new();
        for j in 0..embeddings.len() {
            if i == j {
                continue;
            }
            let cosine_similarity = cosine(&embeddings[i].vector, &embeddings[j].vector)?;
            if cosine_similarity >= threshold {
                scored.push((j, cosine_similarity));
            }
        }
        scored.sort_by(|a, b| {
            b.1.total_cmp(&a.1)
                .then_with(|| embeddings[a.0].task_id.cmp(&embeddings[b.0].task_id))
        });
        for (j, cosine_similarity) in scored.into_iter().take(top_k) {
            if embeddings[i].task_id < embeddings[j].task_id {
                edges.push(PatchGraphEdge {
                    left: embeddings[i].task_id.clone(),
                    right: embeddings[j].task_id.clone(),
                    cosine_similarity,
                });
            }
        }
    }
    edges.sort_by(|a, b| {
        a.left
            .cmp(&b.left)
            .then_with(|| a.right.cmp(&b.right))
            .then_with(|| b.cosine_similarity.total_cmp(&a.cosine_similarity))
    });
    edges.dedup_by(|a, b| a.left == b.left && a.right == b.right);
    Ok(PatchSimilarityGraph {
        node_count: embeddings.len(),
        edge_count: edges.len(),
        threshold,
        top_k,
        edges,
    })
}

fn cosine(a: &[f32], b: &[f32]) -> Result<f32, EvalError> {
    let mut dot = 0.0f32;
    let mut na = 0.0f32;
    let mut nb = 0.0f32;
    for (x, y) in a.iter().zip(b) {
        dot += x * y;
        na += x * x;
        nb += y * y;
    }
    if na <= f32::EPSILON || nb <= f32::EPSILON {
        return Err(EvalError::new(
            EvalErrorCode::InvalidInput,
            "patch graph cannot compare zero-norm embeddings",
        ));
    }
    Ok((dot / (na.sqrt() * nb.sqrt())).clamp(-1.0, 1.0))
}
