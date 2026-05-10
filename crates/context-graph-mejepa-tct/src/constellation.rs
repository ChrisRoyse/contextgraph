use std::collections::BTreeMap;
use std::time::SystemTime;

use bincode::Options;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::TctError;
use crate::freshness;
use crate::shrinkage::ShrinkageOrigin;
use crate::types::{
    validate_cos, ChunkId, CorpusProvenance, EmbedderId, EntityType, Language, MutationCategory,
    OracleOutcome,
};

pub const TCT_SCHEMA_VERSION: u16 = 1;
const NORM_TOLERANCE: f32 = 1.0e-3;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Centroid {
    pub values: Vec<f32>,
    pub sample_count: usize,
    pub origin: ShrinkageOrigin,
}

impl Centroid {
    pub fn try_new(
        values: Vec<f32>,
        sample_count: usize,
        origin: ShrinkageOrigin,
        context: &str,
    ) -> Result<Self, TctError> {
        if sample_count == 0 {
            return Err(TctError::InsufficientSamples {
                cell: context.to_string(),
                observed: 0,
                required: 1,
            });
        }
        validate_centroid_values(&values, context)?;
        Ok(Self {
            values,
            sample_count,
            origin,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Thresholds {
    pub panel_level: BTreeMap<EmbedderId, f32>,
    pub per_chunk_type: BTreeMap<(EntityType, EmbedderId), f32>,
}

impl Thresholds {
    pub fn try_new(
        panel_level: BTreeMap<EmbedderId, f32>,
        per_chunk_type: BTreeMap<(EntityType, EmbedderId), f32>,
    ) -> Result<Self, TctError> {
        if panel_level.len() != EmbedderId::all().len() {
            return Err(TctError::dim(
                EmbedderId::all().len(),
                panel_level.len(),
                "Thresholds.panel_level must cover E1-E21",
            ));
        }
        for embedder in EmbedderId::all() {
            let Some(value) = panel_level.get(&embedder) else {
                return Err(TctError::invalid(
                    "Thresholds.panel_level",
                    format!("missing threshold for {embedder}"),
                ));
            };
            validate_cos("Thresholds.panel_level", *value)?;
        }
        for ((entity_type, embedder), value) in &per_chunk_type {
            validate_cos(
                &format!("Thresholds.per_chunk_type[{entity_type:?},{embedder}]"),
                *value,
            )?;
            let panel = panel_level.get(embedder).ok_or_else(|| {
                TctError::invalid(
                    "Thresholds.per_chunk_type",
                    format!("per-chunk threshold for {embedder} has no panel threshold"),
                )
            })?;
            if value < panel {
                return Err(TctError::ThresholdCalibrationFail {
                    detail: format!(
                        "per-chunk threshold {value} for {entity_type:?}/{embedder} is looser than panel threshold {panel}"
                    ),
                });
            }
        }
        Ok(Self {
            panel_level,
            per_chunk_type,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TctConstellation {
    pub schema_version: u16,
    pub per_category_centroids: BTreeMap<MutationCategory, BTreeMap<EmbedderId, Centroid>>,
    pub per_language_centroids:
        BTreeMap<(Language, MutationCategory), BTreeMap<EmbedderId, Centroid>>,
    pub outcome_centroids: BTreeMap<OracleOutcome, BTreeMap<EmbedderId, Centroid>>,
    pub per_chunk_type_centroids:
        BTreeMap<(MutationCategory, EntityType, Language, EmbedderId), Centroid>,
    pub thresholds: Thresholds,
    pub corpus_provenance: CorpusProvenance,
    pub frozen_at: SystemTime,
    pub version_id: [u8; 32],
}

impl TctConstellation {
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        per_category_centroids: BTreeMap<MutationCategory, BTreeMap<EmbedderId, Centroid>>,
        per_language_centroids: BTreeMap<
            (Language, MutationCategory),
            BTreeMap<EmbedderId, Centroid>,
        >,
        outcome_centroids: BTreeMap<OracleOutcome, BTreeMap<EmbedderId, Centroid>>,
        per_chunk_type_centroids: BTreeMap<
            (MutationCategory, EntityType, Language, EmbedderId),
            Centroid,
        >,
        thresholds: Thresholds,
        corpus_provenance: CorpusProvenance,
        frozen_at: SystemTime,
    ) -> Result<Self, TctError> {
        validate_centroid_map("per_category_centroids", &per_category_centroids)?;
        validate_centroid_map("per_language_centroids", &per_language_centroids)?;
        validate_centroid_map("outcome_centroids", &outcome_centroids)?;
        validate_chunk_centroids(&per_chunk_type_centroids)?;
        if per_category_centroids.is_empty()
            || per_language_centroids.is_empty()
            || outcome_centroids.is_empty()
        {
            return Err(TctError::MissingCentroid {
                detail: "panel-level centroid maps must be non-empty".to_string(),
            });
        }
        let mut value = Self {
            schema_version: TCT_SCHEMA_VERSION,
            per_category_centroids,
            per_language_centroids,
            outcome_centroids,
            per_chunk_type_centroids,
            thresholds,
            corpus_provenance,
            frozen_at,
            version_id: [0u8; 32],
        };
        value.version_id = value.compute_version_id()?;
        Ok(value)
    }

    pub fn version_id(&self) -> [u8; 32] {
        self.version_id
    }

    pub fn compute_version_id(&self) -> Result<[u8; 32], TctError> {
        let payload = VersionPayload {
            schema_version: self.schema_version,
            per_category_centroids: &self.per_category_centroids,
            per_language_centroids: &self.per_language_centroids,
            outcome_centroids: &self.outcome_centroids,
            per_chunk_type_centroids: &self.per_chunk_type_centroids,
            thresholds: &self.thresholds,
            corpus_provenance: &self.corpus_provenance,
            frozen_at: self.frozen_at,
        };
        let bytes = bincode_options().serialize(&payload)?;
        let digest: [u8; 32] = Sha256::digest(bytes).into();
        Ok(digest)
    }

    pub fn validate_integrity(&self) -> Result<(), TctError> {
        if self.schema_version != TCT_SCHEMA_VERSION {
            return Err(TctError::invalid(
                "TctConstellation.schema_version",
                format!(
                    "unsupported schema version {}; expected {TCT_SCHEMA_VERSION}",
                    self.schema_version
                ),
            ));
        }
        validate_centroid_map("per_category_centroids", &self.per_category_centroids)?;
        validate_centroid_map("per_language_centroids", &self.per_language_centroids)?;
        validate_centroid_map("outcome_centroids", &self.outcome_centroids)?;
        validate_chunk_centroids(&self.per_chunk_type_centroids)?;
        let observed = self.compute_version_id()?;
        if observed != self.version_id {
            return Err(TctError::FrozenViolation {
                detail: format!(
                    "version_id mismatch: stored={} recomputed={}",
                    hex::encode(self.version_id),
                    hex::encode(observed)
                ),
            });
        }
        self.check_provenance(&self.corpus_provenance.embedder_versions)?;
        Ok(())
    }

    pub fn lookup_centroid(
        &self,
        mutation: MutationCategory,
        language: Language,
        entity_type: EntityType,
        embedder: EmbedderId,
    ) -> Option<(&[f32], ShrinkageOrigin, usize)> {
        if let Some(centroid) =
            self.per_chunk_type_centroids
                .get(&(mutation, entity_type, language, embedder))
        {
            return Some((
                centroid.values.as_slice(),
                centroid.origin,
                centroid.sample_count,
            ));
        }
        if let Some(by_embedder) = self.per_language_centroids.get(&(language, mutation)) {
            if let Some(centroid) = by_embedder.get(&embedder) {
                return Some((
                    centroid.values.as_slice(),
                    ShrinkageOrigin::LanguageAggregate,
                    centroid.sample_count,
                ));
            }
        }
        let mut entity_values = Vec::new();
        let mut entity_count = 0usize;
        for ((m, e, _l, emb), centroid) in &self.per_chunk_type_centroids {
            if *m == mutation && *e == entity_type && *emb == embedder {
                entity_values.push(centroid.values.as_slice());
                entity_count += centroid.sample_count;
            }
        }
        if entity_values.len() == 1 {
            return Some((
                entity_values[0],
                ShrinkageOrigin::EntityAggregate,
                entity_count,
            ));
        }
        if let Some(by_embedder) = self.per_category_centroids.get(&mutation) {
            if let Some(centroid) = by_embedder.get(&embedder) {
                return Some((
                    centroid.values.as_slice(),
                    ShrinkageOrigin::CategoryAggregate,
                    centroid.sample_count,
                ));
            }
        }
        None
    }

    pub fn panel_centroid(
        &self,
        mutation: MutationCategory,
        embedder: EmbedderId,
    ) -> Option<&Centroid> {
        self.per_category_centroids
            .get(&mutation)
            .and_then(|by_embedder| by_embedder.get(&embedder))
    }

    pub fn threshold(
        &self,
        embedder: EmbedderId,
        entity_type: Option<EntityType>,
    ) -> Result<f32, TctError> {
        if let Some(entity_type) = entity_type {
            if let Some(value) = self.thresholds.per_chunk_type.get(&(entity_type, embedder)) {
                return Ok(*value);
            }
        }
        self.thresholds
            .panel_level
            .get(&embedder)
            .copied()
            .ok_or_else(|| TctError::MissingCentroid {
                detail: format!("missing threshold for {embedder}"),
            })
    }

    pub fn check_provenance(
        &self,
        runtime_embedder_versions: &BTreeMap<EmbedderId, [u8; 32]>,
    ) -> Result<(), TctError> {
        if runtime_embedder_versions.len() != EmbedderId::all().len() {
            return Err(TctError::dim(
                EmbedderId::all().len(),
                runtime_embedder_versions.len(),
                "runtime embedder provenance coverage",
            ));
        }
        for embedder in EmbedderId::all() {
            let expected = self
                .corpus_provenance
                .embedder_versions
                .get(&embedder)
                .ok_or_else(|| {
                    TctError::invalid(
                        "TctConstellation.corpus_provenance.embedder_versions",
                        format!("missing expected digest for {embedder}"),
                    )
                })?;
            let observed =
                runtime_embedder_versions
                    .get(&embedder)
                    .ok_or(TctError::ProvenanceMismatch {
                        embedder,
                        expected: *expected,
                        observed: [0u8; 32],
                    })?;
            if observed != expected {
                return Err(TctError::ProvenanceMismatch {
                    embedder,
                    expected: *expected,
                    observed: *observed,
                });
            }
        }
        Ok(())
    }

    pub fn check_freshness(&self, max_age_days: u32, allow_stale: bool) -> Result<(), TctError> {
        freshness::check_freshness(self, max_age_days, allow_stale)
    }
}

#[derive(Serialize)]
struct VersionPayload<'a> {
    schema_version: u16,
    per_category_centroids: &'a BTreeMap<MutationCategory, BTreeMap<EmbedderId, Centroid>>,
    per_language_centroids:
        &'a BTreeMap<(Language, MutationCategory), BTreeMap<EmbedderId, Centroid>>,
    outcome_centroids: &'a BTreeMap<OracleOutcome, BTreeMap<EmbedderId, Centroid>>,
    per_chunk_type_centroids:
        &'a BTreeMap<(MutationCategory, EntityType, Language, EmbedderId), Centroid>,
    thresholds: &'a Thresholds,
    corpus_provenance: &'a CorpusProvenance,
    frozen_at: SystemTime,
}

pub(crate) fn bincode_options() -> impl bincode::Options {
    bincode::DefaultOptions::new()
        .with_fixint_encoding()
        .with_little_endian()
}

fn validate_centroid_map<K: std::fmt::Debug + Ord>(
    name: &str,
    map: &BTreeMap<K, BTreeMap<EmbedderId, Centroid>>,
) -> Result<(), TctError> {
    for (key, by_embedder) in map {
        if by_embedder.is_empty() {
            return Err(TctError::MissingCentroid {
                detail: format!("{name}[{key:?}] has no embedder centroids"),
            });
        }
        for (embedder, centroid) in by_embedder {
            validate_centroid_values(&centroid.values, &format!("{name}[{key:?}][{embedder}]"))?;
            if centroid.sample_count == 0 {
                return Err(TctError::InsufficientSamples {
                    cell: format!("{name}[{key:?}][{embedder}]"),
                    observed: 0,
                    required: 1,
                });
            }
        }
    }
    Ok(())
}

fn validate_chunk_centroids(
    map: &BTreeMap<(MutationCategory, EntityType, Language, EmbedderId), Centroid>,
) -> Result<(), TctError> {
    for (key, centroid) in map {
        validate_centroid_values(
            &centroid.values,
            &format!("per_chunk_type_centroids[{key:?}]"),
        )?;
        if centroid.sample_count == 0 {
            return Err(TctError::InsufficientSamples {
                cell: format!("per_chunk_type_centroids[{key:?}]"),
                observed: 0,
                required: 1,
            });
        }
    }
    Ok(())
}

pub(crate) fn validate_centroid_values(values: &[f32], context: &str) -> Result<(), TctError> {
    if values.is_empty() {
        return Err(TctError::dim(
            1,
            0,
            format!("{context} centroid must be non-empty"),
        ));
    }
    let mut norm_sq = 0.0f64;
    for (idx, value) in values.iter().enumerate() {
        if !value.is_finite() {
            return Err(TctError::nan(
                context,
                format!("centroid[{idx}] is non-finite: {value}"),
            ));
        }
        norm_sq += (*value as f64) * (*value as f64);
    }
    let norm = norm_sq.sqrt() as f32;
    if (norm - 1.0).abs() > NORM_TOLERANCE {
        return Err(TctError::ConstellationViolation {
            detail: format!("centroid {context} is not L2-normalized; norm={norm:.8}"),
        });
    }
    Ok(())
}

pub(crate) fn normalize(values: &[f32], context: &str) -> Result<Vec<f32>, TctError> {
    if values.is_empty() {
        return Err(TctError::dim(1, 0, format!("{context} empty vector")));
    }
    let mut norm_sq = 0.0f64;
    for (idx, value) in values.iter().enumerate() {
        if !value.is_finite() {
            return Err(TctError::nan(
                context,
                format!("vector[{idx}] is non-finite: {value}"),
            ));
        }
        norm_sq += (*value as f64) * (*value as f64);
    }
    let norm = norm_sq.sqrt();
    if norm < 1.0e-12 {
        return Err(TctError::ConstellationViolation {
            detail: format!("zero-norm centroid source at {context}"),
        });
    }
    let out = values
        .iter()
        .map(|value| (*value as f64 / norm) as f32)
        .collect::<Vec<_>>();
    validate_centroid_values(&out, context)?;
    Ok(out)
}

pub fn chunk_id_for_test(entity_type: EntityType) -> Result<ChunkId, TctError> {
    ChunkId::try_new([1u8; 32], Language::Python, entity_type, 1, 10, [2u8; 16])
}
