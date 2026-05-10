use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::constellation::TctConstellation;
use crate::shrinkage::ShrinkageOrigin;
use crate::shrinkage_engine::{ShrinkageDecision, SHRINKAGE_FULL_THRESHOLD, SHRINKAGE_HARD_FLOOR};
use crate::types::{EmbedderId, EntityType};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConstellationInspectOutput {
    pub version_id: String,
    pub corpus_sha: String,
    pub embedder_versions: BTreeMap<EmbedderId, String>,
    pub frozen_at: String,
    pub code_version: String,
    pub cell_counts: CellCounts,
    pub sample_support_histogram: SampleSupportHistogram,
    pub thresholds: BTreeMap<EmbedderId, f32>,
    pub per_chunk_type_threshold_summary: BTreeMap<EntityType, EntityThresholdSummary>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CellCounts {
    pub panel_level_cells: u32,
    pub per_chunk_type_cells: u32,
    pub shrunken_cells: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SampleSupportHistogram {
    pub n_lt_5: u32,
    pub n_5_to_50: u32,
    pub n_50_plus: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EntityThresholdSummary {
    pub mean_threshold: f32,
    pub n_cells: u32,
}

pub fn build_inspect_summary(
    constellation: &TctConstellation,
    shrinkage_decisions: Option<&[ShrinkageDecision]>,
) -> ConstellationInspectOutput {
    let panel_level_cells = constellation
        .per_category_centroids
        .values()
        .map(|m| m.len() as u32)
        .sum::<u32>()
        + constellation
            .per_language_centroids
            .values()
            .map(|m| m.len() as u32)
            .sum::<u32>()
        + constellation
            .outcome_centroids
            .values()
            .map(|m| m.len() as u32)
            .sum::<u32>();
    let per_chunk_type_cells = constellation.per_chunk_type_centroids.len() as u32;
    let shrunken_cells = shrinkage_decisions
        .map(|items| {
            items
                .iter()
                .filter(|item| item.origin != ShrinkageOrigin::OwnCell)
                .count() as u32
        })
        .unwrap_or(0);
    let mut hist = SampleSupportHistogram {
        n_lt_5: 0,
        n_5_to_50: 0,
        n_50_plus: 0,
    };
    if let Some(decisions) = shrinkage_decisions {
        for decision in decisions {
            if decision.observed_n < SHRINKAGE_HARD_FLOOR {
                hist.n_lt_5 += 1;
            } else if decision.observed_n < SHRINKAGE_FULL_THRESHOLD {
                hist.n_5_to_50 += 1;
            } else {
                hist.n_50_plus += 1;
            }
        }
    } else {
        for centroid in constellation.per_chunk_type_centroids.values() {
            if centroid.sample_count < SHRINKAGE_HARD_FLOOR {
                hist.n_lt_5 += 1;
            } else if centroid.sample_count < SHRINKAGE_FULL_THRESHOLD {
                hist.n_5_to_50 += 1;
            } else {
                hist.n_50_plus += 1;
            }
        }
    }
    let mut by_entity: BTreeMap<EntityType, Vec<f32>> = BTreeMap::new();
    for ((entity_type, _embedder), value) in &constellation.thresholds.per_chunk_type {
        by_entity.entry(*entity_type).or_default().push(*value);
    }
    let per_chunk_type_threshold_summary = by_entity
        .into_iter()
        .map(|(entity_type, values)| {
            let mean_threshold = values.iter().sum::<f32>() / values.len() as f32;
            (
                entity_type,
                EntityThresholdSummary {
                    mean_threshold,
                    n_cells: values.len() as u32,
                },
            )
        })
        .collect();
    ConstellationInspectOutput {
        version_id: hex::encode(constellation.version_id),
        corpus_sha: hex::encode(constellation.corpus_provenance.corpus_sha),
        embedder_versions: constellation
            .corpus_provenance
            .embedder_versions
            .iter()
            .map(|(embedder, sha)| (*embedder, hex::encode(sha)))
            .collect(),
        frozen_at: chrono::DateTime::<chrono::Utc>::from(constellation.frozen_at).to_rfc3339(),
        code_version: constellation.corpus_provenance.code_version.clone(),
        cell_counts: CellCounts {
            panel_level_cells,
            per_chunk_type_cells,
            shrunken_cells,
        },
        sample_support_histogram: hist,
        thresholds: constellation.thresholds.panel_level.clone(),
        per_chunk_type_threshold_summary,
    }
}
