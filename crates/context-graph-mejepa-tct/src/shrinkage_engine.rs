use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::builder::{centroid_from_bucket, CentroidBucket, PanelLevelCentroids};
use crate::constellation::{normalize, Centroid};
use crate::error::TctError;
use crate::shrinkage::ShrinkageOrigin;
use crate::types::{EmbedderId, EntityType, Language, MutationCategory};

pub const SHRINKAGE_FULL_THRESHOLD: usize = 50;
pub const SHRINKAGE_HARD_FLOOR: usize = 5;

pub type ChunkCell = (MutationCategory, EntityType, Language, EmbedderId);
pub type ChunkBucketMap = BTreeMap<ChunkCell, CentroidBucket>;
pub type ChunkCentroidMap = BTreeMap<ChunkCell, Centroid>;
pub type ShrinkageResult = (ChunkCentroidMap, Vec<ShrinkageDecision>);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShrinkageDecision {
    pub cell: (MutationCategory, EntityType, Language, EmbedderId),
    pub origin: ShrinkageOrigin,
    pub observed_n: usize,
}

pub fn apply_shrinkage(
    per_chunk_type_buckets: &ChunkBucketMap,
    panel_level: &PanelLevelCentroids,
) -> Result<ShrinkageResult, TctError> {
    let mut out = BTreeMap::new();
    let mut decisions = Vec::new();
    for (cell, bucket) in per_chunk_type_buckets {
        let (mutation, entity_type, language, embedder) = *cell;
        let (centroid, origin) = if bucket.count >= SHRINKAGE_FULL_THRESHOLD {
            (
                centroid_from_bucket(bucket, ShrinkageOrigin::OwnCell, &format!("chunk/{cell:?}"))?,
                ShrinkageOrigin::OwnCell,
            )
        } else if bucket.count >= SHRINKAGE_HARD_FLOOR {
            if let Some(centroid) = panel_level
                .per_language_centroids
                .get(&(language, mutation))
                .and_then(|by_embedder| by_embedder.get(&embedder))
            {
                (
                    Centroid::try_new(
                        centroid.values.clone(),
                        bucket.count,
                        ShrinkageOrigin::LanguageAggregate,
                        &format!("chunk-shrunk-language/{cell:?}"),
                    )?,
                    ShrinkageOrigin::LanguageAggregate,
                )
            } else if let Some(centroid) =
                entity_aggregate(per_chunk_type_buckets, mutation, entity_type, embedder)?
            {
                (centroid, ShrinkageOrigin::EntityAggregate)
            } else {
                let centroid = panel_level
                    .per_category_centroids
                    .get(&mutation)
                    .and_then(|by_embedder| by_embedder.get(&embedder))
                    .ok_or_else(|| TctError::MissingCentroid {
                        detail: format!("no category aggregate for {mutation:?}/{embedder}"),
                    })?;
                (
                    Centroid::try_new(
                        centroid.values.clone(),
                        bucket.count,
                        ShrinkageOrigin::CategoryAggregate,
                        &format!("chunk-shrunk-category/{cell:?}"),
                    )?,
                    ShrinkageOrigin::CategoryAggregate,
                )
            }
        } else {
            return Err(TctError::InsufficientSamples {
                cell: format!("{cell:?}"),
                observed: bucket.count,
                required: SHRINKAGE_HARD_FLOOR,
            });
        };
        out.insert(*cell, centroid);
        decisions.push(ShrinkageDecision {
            cell: *cell,
            origin,
            observed_n: bucket.count,
        });
    }
    Ok((out, decisions))
}

fn entity_aggregate(
    per_chunk_type_buckets: &ChunkBucketMap,
    mutation: MutationCategory,
    entity_type: EntityType,
    embedder: EmbedderId,
) -> Result<Option<Centroid>, TctError> {
    let mut sum = Vec::<f32>::new();
    let mut count = 0usize;
    for ((m, e, _language, emb), bucket) in per_chunk_type_buckets {
        if *m != mutation || *e != entity_type || *emb != embedder {
            continue;
        }
        if sum.is_empty() {
            sum = vec![0.0; bucket.sum.len()];
        }
        if sum.len() != bucket.sum.len() {
            return Err(TctError::dim(
                sum.len(),
                bucket.sum.len(),
                "entity aggregate bucket dimension mismatch",
            ));
        }
        for (dst, src) in sum.iter_mut().zip(&bucket.sum) {
            *dst += *src;
        }
        count += bucket.count;
    }
    if count < SHRINKAGE_FULL_THRESHOLD {
        return Ok(None);
    }
    let mean = sum
        .iter()
        .map(|value| *value / count as f32)
        .collect::<Vec<_>>();
    let values = normalize(
        &mean,
        &format!("entity-aggregate/{mutation:?}/{entity_type:?}/{embedder}"),
    )?;
    Ok(Some(Centroid::try_new(
        values,
        count,
        ShrinkageOrigin::EntityAggregate,
        &format!("entity-aggregate/{mutation:?}/{entity_type:?}/{embedder}"),
    )?))
}
