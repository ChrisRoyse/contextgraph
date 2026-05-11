use super::{non_empty, validate_finite, UtmlError, UtmlErrorCode};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const REQUIRED_EMBEDDER_COUNT: usize = 14;
pub const BOOTSTRAP_NEUTRAL_COHERENCE: f32 = 0.5;

pub trait ContentEmbedder {
    fn id(&self) -> &str;
    fn embed(&self, text: &str) -> Result<Vec<f32>, UtmlError>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmbedderCoherenceReport {
    pub coherence: f32,
    pub corpus_size: usize,
    pub k: usize,
    pub bootstrap_neutral: bool,
    pub reason: Option<String>,
    pub pairwise_jaccard: BTreeMap<String, f32>,
}

pub fn compute_embedder_coherence(
    embedders: &[Box<dyn ContentEmbedder>],
    corpus: &[String],
    k: usize,
    bootstrap_threshold: usize,
) -> Result<EmbedderCoherenceReport, UtmlError> {
    if embedders.len() != REQUIRED_EMBEDDER_COUNT {
        return Err(UtmlError::new(
            UtmlErrorCode::InvalidSignal,
            format!(
                "embedder coherence requires exactly {REQUIRED_EMBEDDER_COUNT} embedders; got {}",
                embedders.len()
            ),
        ));
    }
    non_empty("corpus", corpus)?;
    if k == 0 {
        return Err(UtmlError::new(
            UtmlErrorCode::OutOfRange,
            "embedder coherence k must be greater than zero",
        ));
    }
    if corpus.len() <= k {
        return Err(UtmlError::new(
            UtmlErrorCode::OutOfRange,
            format!(
                "embedder coherence corpus size {} must be greater than k={k}",
                corpus.len()
            ),
        ));
    }
    if corpus.len() < bootstrap_threshold {
        return Ok(EmbedderCoherenceReport {
            coherence: BOOTSTRAP_NEUTRAL_COHERENCE,
            corpus_size: corpus.len(),
            k,
            bootstrap_neutral: true,
            reason: Some(format!(
                "bootstrap_neutral_n_lt_{bootstrap_threshold}: corpus_size={}",
                corpus.len()
            )),
            pairwise_jaccard: BTreeMap::new(),
        });
    }

    let mut neighbor_sets = Vec::with_capacity(embedders.len());
    for embedder in embedders {
        let embeddings = embed_corpus(embedder.as_ref(), corpus)?;
        neighbor_sets.push((embedder.id().to_string(), top_k_neighbors(&embeddings, k)?));
    }

    let mut pairwise_jaccard = BTreeMap::new();
    let mut sum = 0.0f32;
    let mut pair_count = 0usize;
    for i in 0..neighbor_sets.len() {
        for j in (i + 1)..neighbor_sets.len() {
            let score = mean_neighbor_jaccard(&neighbor_sets[i].1, &neighbor_sets[j].1)?;
            let key = format!("{}::{}", neighbor_sets[i].0, neighbor_sets[j].0);
            pairwise_jaccard.insert(key, score);
            sum += score;
            pair_count += 1;
        }
    }
    if pair_count == 0 {
        return Err(UtmlError::new(
            UtmlErrorCode::InvalidSignal,
            "embedder coherence produced zero embedder pairs",
        ));
    }
    Ok(EmbedderCoherenceReport {
        coherence: (sum / pair_count as f32).clamp(0.0, 1.0),
        corpus_size: corpus.len(),
        k,
        bootstrap_neutral: false,
        reason: None,
        pairwise_jaccard,
    })
}

pub fn jaccard(a: &[usize], b: &[usize]) -> Result<f32, UtmlError> {
    non_empty("jaccard.a", a)?;
    non_empty("jaccard.b", b)?;
    let a_set = a.iter().copied().collect::<std::collections::BTreeSet<_>>();
    let b_set = b.iter().copied().collect::<std::collections::BTreeSet<_>>();
    let intersection = a_set.intersection(&b_set).count();
    let union = a_set.union(&b_set).count();
    if union == 0 {
        return Err(UtmlError::new(
            UtmlErrorCode::InvalidSignal,
            "jaccard union is empty",
        ));
    }
    Ok(intersection as f32 / union as f32)
}

fn embed_corpus(
    embedder: &dyn ContentEmbedder,
    corpus: &[String],
) -> Result<Vec<Vec<f32>>, UtmlError> {
    let mut out = Vec::with_capacity(corpus.len());
    let mut dim = None;
    for (idx, text) in corpus.iter().enumerate() {
        if text.trim().is_empty() {
            return Err(UtmlError::new(
                UtmlErrorCode::InvalidSignal,
                format!("corpus[{idx}] is empty"),
            ));
        }
        let vector = embedder.embed(text)?;
        non_empty("embedder vector", &vector)?;
        if let Some(expected) = dim {
            if vector.len() != expected {
                return Err(UtmlError::new(
                    UtmlErrorCode::InvalidSignal,
                    format!(
                        "embedder {} returned dim {}; expected {}",
                        embedder.id(),
                        vector.len(),
                        expected
                    ),
                ));
            }
        } else {
            dim = Some(vector.len());
        }
        for (component_idx, value) in vector.iter().enumerate() {
            validate_finite(
                &format!("embedder.{}.vector[{idx}][{component_idx}]", embedder.id()),
                *value,
            )?;
        }
        out.push(vector);
    }
    Ok(out)
}

fn top_k_neighbors(embeddings: &[Vec<f32>], k: usize) -> Result<Vec<Vec<usize>>, UtmlError> {
    if embeddings.len() <= k {
        return Err(UtmlError::new(
            UtmlErrorCode::OutOfRange,
            "top_k_neighbors requires more rows than k",
        ));
    }
    let mut out = Vec::with_capacity(embeddings.len());
    for i in 0..embeddings.len() {
        let mut scored = Vec::with_capacity(embeddings.len() - 1);
        for j in 0..embeddings.len() {
            if i == j {
                continue;
            }
            scored.push((j, cosine(&embeddings[i], &embeddings[j])?));
        }
        scored.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        out.push(scored.into_iter().take(k).map(|(idx, _)| idx).collect());
    }
    Ok(out)
}

fn mean_neighbor_jaccard(a: &[Vec<usize>], b: &[Vec<usize>]) -> Result<f32, UtmlError> {
    if a.len() != b.len() || a.is_empty() {
        return Err(UtmlError::new(
            UtmlErrorCode::InvalidSignal,
            "neighbor lists must have same non-empty row count",
        ));
    }
    let mut sum = 0.0f32;
    for (left, right) in a.iter().zip(b) {
        sum += jaccard(left, right)?;
    }
    Ok(sum / a.len() as f32)
}

fn cosine(a: &[f32], b: &[f32]) -> Result<f32, UtmlError> {
    if a.len() != b.len() || a.is_empty() {
        return Err(UtmlError::new(
            UtmlErrorCode::InvalidSignal,
            "cosine requires same non-empty dimensions",
        ));
    }
    let mut dot = 0.0f32;
    let mut na = 0.0f32;
    let mut nb = 0.0f32;
    for (idx, (x, y)) in a.iter().zip(b).enumerate() {
        validate_finite(&format!("cosine.a[{idx}]"), *x)?;
        validate_finite(&format!("cosine.b[{idx}]"), *y)?;
        dot += x * y;
        na += x * x;
        nb += y * y;
    }
    if na <= f32::EPSILON || nb <= f32::EPSILON {
        return Err(UtmlError::new(
            UtmlErrorCode::InvalidSignal,
            "cosine cannot compare zero-norm vectors",
        ));
    }
    Ok(dot / (na.sqrt() * nb.sqrt()))
}
