use crate::embedder_id::EmbedderId;
use crate::error::{EmbedError, EmbedResult};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoherenceReport {
    pub pair_count: usize,
    pub mean_jaccard: f32,
    pub per_pair: BTreeMap<String, f32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CollapseReport {
    pub embedder_count: usize,
    pub mean_similarity: f32,
    pub std_similarity: f32,
    pub collapse_score: f32,
}

pub fn embedder_coherence(
    neighbors: &BTreeMap<EmbedderId, Vec<String>>,
) -> EmbedResult<CoherenceReport> {
    if neighbors.len() < 2 {
        return Err(EmbedError::invalid(
            "embedder_coherence.neighbors",
            "at least two embedder neighbor sets are required",
            "run retrieval for two or more embedders before computing coherence",
        ));
    }
    for (embedder, ids) in neighbors {
        if ids.is_empty() {
            return Err(EmbedError::invalid(
                "embedder_coherence.neighbors",
                format!("{embedder} neighbor set is empty"),
                "compute nearest neighbors before coherence scoring",
            ));
        }
    }
    let keys = neighbors.keys().copied().collect::<Vec<_>>();
    let mut per_pair = BTreeMap::new();
    let mut total = 0.0f32;
    let mut count = 0usize;
    for i in 0..keys.len() {
        for j in (i + 1)..keys.len() {
            let a = &neighbors[&keys[i]];
            let b = &neighbors[&keys[j]];
            let score = jaccard(a, b);
            per_pair.insert(format!("{}:{}", keys[i], keys[j]), score);
            total += score;
            count += 1;
        }
    }
    Ok(CoherenceReport {
        pair_count: count,
        mean_jaccard: total / count as f32,
        per_pair,
    })
}

pub fn predictor_collapse_score(
    similarities: &BTreeMap<EmbedderId, f32>,
) -> EmbedResult<CollapseReport> {
    if similarities.len() < 2 {
        return Err(EmbedError::invalid(
            "predictor_collapse_score.similarities",
            "at least two embedder similarities are required",
            "score predicted-vs-target agreement under two or more embedder spaces",
        ));
    }
    for (embedder, value) in similarities {
        if !value.is_finite() || !(-1.0..=1.0).contains(value) {
            return Err(EmbedError::invalid(
                "predictor_collapse_score.similarities",
                format!("{embedder} similarity {value} is outside [-1,1]"),
                "normalize cosine similarities before collapse scoring",
            ));
        }
    }
    let mean = similarities.values().sum::<f32>() / similarities.len() as f32;
    let variance = similarities
        .values()
        .map(|value| {
            let delta = *value - mean;
            delta * delta
        })
        .sum::<f32>()
        / similarities.len() as f32;
    let std = variance.sqrt();
    Ok(CollapseReport {
        embedder_count: similarities.len(),
        mean_similarity: mean,
        std_similarity: std,
        collapse_score: (1.0 - std).clamp(0.0, 1.0),
    })
}

fn jaccard(a: &[String], b: &[String]) -> f32 {
    let mut intersection = 0usize;
    for id in a {
        if b.contains(id) {
            intersection += 1;
        }
    }
    let union = a.len() + b.len() - intersection;
    if union == 0 {
        0.0
    } else {
        intersection as f32 / union as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coherence_and_collapse_use_observed_scores() {
        let neighbors = BTreeMap::from([
            (EmbedderId::E1, vec!["a".into(), "b".into(), "c".into()]),
            (EmbedderId::E7, vec!["b".into(), "c".into(), "d".into()]),
        ]);
        let coherence = embedder_coherence(&neighbors).unwrap();
        assert_eq!(coherence.pair_count, 1);
        assert!(coherence.mean_jaccard > 0.49 && coherence.mean_jaccard < 0.51);

        let collapse = predictor_collapse_score(&BTreeMap::from([
            (EmbedderId::E1, 0.9),
            (EmbedderId::E7, 0.4),
            (EmbedderId::E14, 0.2),
        ]))
        .unwrap();
        assert!(collapse.std_similarity > 0.0);
    }
}
