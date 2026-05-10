use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::error::TctError;
use crate::shrinkage::ShrinkageOrigin;
use crate::types::{ChunkId, EmbedderId, GtauOutput};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConstellationHingeOutput {
    pub per_embedder_hinges: BTreeMap<EmbedderId, f32>,
    pub total_hinge: f32,
    pub centroid_origin: BTreeMap<EmbedderId, ShrinkageOrigin>,
}

impl ConstellationHingeOutput {
    pub fn try_new(
        per_embedder_hinges: BTreeMap<EmbedderId, f32>,
        centroid_origin: BTreeMap<EmbedderId, ShrinkageOrigin>,
    ) -> Result<Self, TctError> {
        if per_embedder_hinges.len() != EmbedderId::all().len() {
            return Err(TctError::dim(
                EmbedderId::all().len(),
                per_embedder_hinges.len(),
                "ConstellationHingeOutput.per_embedder_hinges",
            ));
        }
        if centroid_origin.len() != EmbedderId::all().len() {
            return Err(TctError::dim(
                EmbedderId::all().len(),
                centroid_origin.len(),
                "ConstellationHingeOutput.centroid_origin",
            ));
        }
        let mut total_hinge = 0.0f32;
        for (embedder, value) in &per_embedder_hinges {
            if !value.is_finite() || *value < 0.0 {
                return Err(TctError::nan(
                    "ConstellationHingeOutput.per_embedder_hinges",
                    format!("{embedder} hinge must be finite and non-negative, got {value}"),
                ));
            }
            total_hinge += *value;
        }
        if !total_hinge.is_finite() {
            return Err(TctError::nan(
                "ConstellationHingeOutput.total_hinge",
                format!("total hinge is {total_hinge}"),
            ));
        }
        Ok(Self {
            per_embedder_hinges,
            total_hinge,
            centroid_origin,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChunkGtauOutput {
    pub per_chunk_results: Vec<(ChunkId, GtauOutput)>,
    pub violating_chunks: Vec<ChunkId>,
    pub aggregate_satisfied: bool,
}

impl ChunkGtauOutput {
    pub fn try_new(
        per_chunk_results: Vec<(ChunkId, GtauOutput)>,
        violating_chunks: Vec<ChunkId>,
    ) -> Result<Self, TctError> {
        if per_chunk_results.is_empty() {
            return Err(TctError::InsufficientSamples {
                cell: "ChunkGtauOutput.per_chunk_results".to_string(),
                observed: 0,
                required: 1,
            });
        }
        Ok(Self {
            aggregate_satisfied: violating_chunks.is_empty(),
            per_chunk_results,
            violating_chunks,
        })
    }
}
