use std::collections::BTreeMap;
use std::time::SystemTime;

use context_graph_mejepa_instruments::Panel;

use crate::constellation::{normalize, Centroid, TctConstellation, Thresholds};
use crate::error::TctError;
use crate::panel_slots::panel_slice_for_embedder;
use crate::shrinkage::ShrinkageOrigin;
use crate::shrinkage_engine::{apply_shrinkage, ShrinkageDecision};
use crate::types::{
    validate_code_version, ChunkId, CorpusProvenance, EmbedderId, EntityType, Language,
    MutationCategory, OracleOutcome,
};

#[derive(Debug, Clone, PartialEq)]
pub struct CentroidBucket {
    pub sum: Vec<f32>,
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PanelLevelCentroids {
    pub per_category_centroids: BTreeMap<MutationCategory, BTreeMap<EmbedderId, Centroid>>,
    pub per_language_centroids:
        BTreeMap<(Language, MutationCategory), BTreeMap<EmbedderId, Centroid>>,
    pub outcome_centroids: BTreeMap<OracleOutcome, BTreeMap<EmbedderId, Centroid>>,
    pub per_chunk_type_buckets:
        BTreeMap<(MutationCategory, EntityType, Language, EmbedderId), CentroidBucket>,
}

pub struct ConstellationBuilder {
    corpus_sha: [u8; 32],
    embedder_versions: BTreeMap<EmbedderId, [u8; 32]>,
    code_version: String,
    per_category_buckets: BTreeMap<(MutationCategory, EmbedderId), CentroidBucket>,
    per_language_buckets: BTreeMap<(Language, MutationCategory, EmbedderId), CentroidBucket>,
    outcome_buckets: BTreeMap<(OracleOutcome, EmbedderId), CentroidBucket>,
    per_chunk_type_buckets:
        BTreeMap<(MutationCategory, EntityType, Language, EmbedderId), CentroidBucket>,
}

impl ConstellationBuilder {
    pub fn new(
        corpus_sha: [u8; 32],
        embedder_versions: BTreeMap<EmbedderId, [u8; 32]>,
        code_version: String,
    ) -> Result<Self, TctError> {
        if embedder_versions.len() != EmbedderId::all().len() {
            return Err(TctError::dim(
                EmbedderId::all().len(),
                embedder_versions.len(),
                "ConstellationBuilder.embedder_versions",
            ));
        }
        for embedder in EmbedderId::all() {
            if !embedder_versions.contains_key(&embedder) {
                return Err(TctError::invalid(
                    "ConstellationBuilder.embedder_versions",
                    format!("missing digest for {embedder}"),
                ));
            }
        }
        validate_code_version(&code_version)?;
        Ok(Self {
            corpus_sha,
            embedder_versions,
            code_version,
            per_category_buckets: BTreeMap::new(),
            per_language_buckets: BTreeMap::new(),
            outcome_buckets: BTreeMap::new(),
            per_chunk_type_buckets: BTreeMap::new(),
        })
    }

    pub fn ingest_corpus_entry(
        &mut self,
        panel: &Panel,
        mutation: MutationCategory,
        oracle_outcome: OracleOutcome,
        language: Language,
        _panel_entity_type: EntityType,
        chunk_panels: &[(ChunkId, Panel)],
    ) -> Result<(), TctError> {
        for embedder in EmbedderId::all() {
            let slot = panel_slice_for_embedder(panel, embedder)?;
            accumulate(
                &mut self.per_category_buckets,
                (mutation, embedder),
                slot,
                "per_category_buckets",
            )?;
            accumulate(
                &mut self.per_language_buckets,
                (language, mutation, embedder),
                slot,
                "per_language_buckets",
            )?;
            accumulate(
                &mut self.outcome_buckets,
                (oracle_outcome, embedder),
                slot,
                "outcome_buckets",
            )?;
        }
        for (chunk_id, chunk_panel) in chunk_panels {
            if chunk_id.language != language {
                return Err(TctError::invalid(
                    "chunk_id.language",
                    format!(
                        "chunk language {:?} does not match corpus entry language {:?}",
                        chunk_id.language, language
                    ),
                ));
            }
            for embedder in EmbedderId::all() {
                let slot = panel_slice_for_embedder(chunk_panel, embedder)?;
                accumulate(
                    &mut self.per_chunk_type_buckets,
                    (mutation, chunk_id.entity_type, language, embedder),
                    slot,
                    "per_chunk_type_buckets",
                )?;
            }
        }
        Ok(())
    }

    pub fn finalize_panel_level(&self) -> Result<PanelLevelCentroids, TctError> {
        let mut per_category_centroids: BTreeMap<MutationCategory, BTreeMap<EmbedderId, Centroid>> =
            BTreeMap::new();
        for ((mutation, embedder), bucket) in &self.per_category_buckets {
            let centroid = centroid_from_bucket(
                bucket,
                ShrinkageOrigin::CategoryAggregate,
                &format!("category/{mutation:?}/{embedder}"),
            )?;
            per_category_centroids
                .entry(*mutation)
                .or_default()
                .insert(*embedder, centroid);
        }

        let mut per_language_centroids: BTreeMap<
            (Language, MutationCategory),
            BTreeMap<EmbedderId, Centroid>,
        > = BTreeMap::new();
        for ((language, mutation, embedder), bucket) in &self.per_language_buckets {
            let centroid = centroid_from_bucket(
                bucket,
                ShrinkageOrigin::LanguageAggregate,
                &format!("language/{language:?}/{mutation:?}/{embedder}"),
            )?;
            per_language_centroids
                .entry((*language, *mutation))
                .or_default()
                .insert(*embedder, centroid);
        }

        let mut outcome_centroids: BTreeMap<OracleOutcome, BTreeMap<EmbedderId, Centroid>> =
            BTreeMap::new();
        for ((outcome, embedder), bucket) in &self.outcome_buckets {
            let centroid = centroid_from_bucket(
                bucket,
                ShrinkageOrigin::CategoryAggregate,
                &format!("outcome/{outcome:?}/{embedder}"),
            )?;
            outcome_centroids
                .entry(*outcome)
                .or_default()
                .insert(*embedder, centroid);
        }

        Ok(PanelLevelCentroids {
            per_category_centroids,
            per_language_centroids,
            outcome_centroids,
            per_chunk_type_buckets: self.per_chunk_type_buckets.clone(),
        })
    }

    pub fn finalize(
        self,
        thresholds: Thresholds,
        frozen_at: SystemTime,
    ) -> Result<(TctConstellation, Vec<ShrinkageDecision>), TctError> {
        let panel_level = self.finalize_panel_level()?;
        let (per_chunk_type_centroids, decisions) =
            apply_shrinkage(&panel_level.per_chunk_type_buckets, &panel_level)?;
        let provenance = CorpusProvenance::try_new(
            self.corpus_sha,
            self.embedder_versions,
            frozen_at,
            self.code_version,
        )?;
        let constellation = TctConstellation::try_new(
            panel_level.per_category_centroids,
            panel_level.per_language_centroids,
            panel_level.outcome_centroids,
            per_chunk_type_centroids,
            thresholds,
            provenance,
            frozen_at,
        )?;
        Ok((constellation, decisions))
    }
}

pub(crate) fn centroid_from_bucket(
    bucket: &CentroidBucket,
    origin: ShrinkageOrigin,
    context: &str,
) -> Result<Centroid, TctError> {
    if bucket.count == 0 {
        return Err(TctError::InsufficientSamples {
            cell: context.to_string(),
            observed: 0,
            required: 1,
        });
    }
    let mean = bucket
        .sum
        .iter()
        .map(|value| *value / bucket.count as f32)
        .collect::<Vec<_>>();
    let values = normalize(&mean, context)?;
    Centroid::try_new(values, bucket.count, origin, context)
}

pub(crate) fn accumulate<K: Ord>(
    buckets: &mut BTreeMap<K, CentroidBucket>,
    key: K,
    slot: &[f32],
    context: &str,
) -> Result<(), TctError> {
    if slot.is_empty() {
        return Err(TctError::dim(1, 0, format!("{context} empty slot")));
    }
    for (idx, value) in slot.iter().enumerate() {
        if !value.is_finite() {
            return Err(TctError::nan(
                context,
                format!("slot[{idx}] is non-finite: {value}"),
            ));
        }
    }
    let entry = buckets.entry(key).or_insert_with(|| CentroidBucket {
        sum: vec![0.0; slot.len()],
        count: 0,
    });
    if entry.sum.len() != slot.len() {
        return Err(TctError::dim(
            entry.sum.len(),
            slot.len(),
            format!("{context} mixed dimensions"),
        ));
    }
    for (dst, src) in entry.sum.iter_mut().zip(slot) {
        *dst += *src;
    }
    entry.count += 1;
    Ok(())
}
