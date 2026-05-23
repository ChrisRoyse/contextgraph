use std::collections::BTreeMap;

use context_graph_mejepa_cf::{CF_MEJEPA_DDA_SIGNALS, CF_MEJEPA_HEAD_PROJECTION_REGISTRY};
use rocksdb::{IteratorMode, WriteOptions, DB};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::calibration::cf;
use crate::error::MejepaInferError;
use crate::types::{ChunkId, DdaSignals, EmbedderId, HeadId, PanelId};

pub const HEAD_PROJECTION_SCHEMA_VERSION: u32 = 1;
pub const HEAD_PROJECTION_SCHEMA: &str = "head-dda-projection-v1";
pub const HEAD_PROJECTION_GEOMETRY_SCALAR_SCHEMA_VERSION: u32 = 1;
pub const HEAD_PROJECTION_GEOMETRY_SCALAR_SCHEMA: &str =
    "head-projection-geometry-scalar-contract-v1";
pub const HEAD_PROJECTION_GEOMETRY_SCALAR_PREFIX: &str = "geometry:";
pub const HEAD_PROJECTION_ALLOWED_GEOMETRY_SCALARS: &[&str] = &[
    "geometry:label_quality_signal_minus_noise",
    "geometry:label_quality_mean_signal",
    "geometry:label_quality_mean_noise",
    "geometry:label_quality_live_label_ratio",
    "geometry:label_quality_noise_label_ratio",
    "geometry:downstream_weak_cell_regression_flag",
    "geometry:prediction_promotion_allowed",
    "geometry:missingness_or_quarantine_flag",
    "geometry:boundary_or_ood_score",
    "geometry:valid_space_consensus_score",
];

pub const E_ORACLE: &str = "E_Oracle";
pub const E_TEST: &str = "E_Test";
pub const E_REASONING: &str = "E_Reasoning";
pub const E_AST: &str = "E_AST";
pub const E_CFG: &str = "E_CFG";
pub const E_DATA_FLOW: &str = "E_DataFlow";
pub const E_TYPE_GRAPH: &str = "E_TypeGraph";
pub const E_STATIC_ANALYSIS: &str = "E_StaticAnalysis";
pub const E_DIFF: &str = "E_Diff";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum HeadProjectionFeatureSource {
    PerEmbedderCosine { embedder_id: EmbedderId },
    PairwiseCosine { left: EmbedderId, right: EmbedderId },
    Scalar { name: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HeadProjectionFeatureSpec {
    pub name: String,
    pub source: HeadProjectionFeatureSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HeadProjectionSpec {
    pub schema: String,
    pub schema_version: u32,
    pub head: HeadId,
    pub features: Vec<HeadProjectionFeatureSpec>,
}

impl HeadProjectionSpec {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        if self.schema != HEAD_PROJECTION_SCHEMA {
            return Err(MejepaInferError::HeadProjectionSchemaMismatch {
                expected: HEAD_PROJECTION_SCHEMA.to_string(),
                actual: self.schema.clone(),
            });
        }
        if self.schema_version != HEAD_PROJECTION_SCHEMA_VERSION {
            return Err(MejepaInferError::HeadProjectionSchemaMismatch {
                expected: HEAD_PROJECTION_SCHEMA_VERSION.to_string(),
                actual: self.schema_version.to_string(),
            });
        }
        if self.features.is_empty() {
            return Err(MejepaInferError::InvalidInput {
                field: "head_projection.features".to_string(),
                detail: format!("{} head has no projected DDA features", self.head.as_str()),
            });
        }
        for (idx, feature) in self.features.iter().enumerate() {
            if feature.name.trim().is_empty() {
                return Err(MejepaInferError::InvalidInput {
                    field: format!("head_projection.features[{idx}].name"),
                    detail: "feature name must be non-empty".to_string(),
                });
            }
            if let HeadProjectionFeatureSource::Scalar { name } = &feature.source {
                validate_head_projection_scalar_feature_name(name)?;
            }
        }
        Ok(())
    }

    pub fn feature_schema_hash(&self) -> Result<String, MejepaInferError> {
        self.validate()?;
        let bytes = serde_json::to_vec(self)?;
        Ok(hex::encode(Sha256::digest(bytes)))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HeadProjectionFeatureValue {
    pub name: String,
    pub value: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HeadProjection {
    pub schema: String,
    pub schema_version: u32,
    pub panel_id: PanelId,
    pub chunk_id: ChunkId,
    pub head: HeadId,
    pub features: Vec<HeadProjectionFeatureValue>,
}

impl HeadProjection {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        if self.schema != HEAD_PROJECTION_SCHEMA {
            return Err(MejepaInferError::HeadProjectionSchemaMismatch {
                expected: HEAD_PROJECTION_SCHEMA.to_string(),
                actual: self.schema.clone(),
            });
        }
        if self.schema_version != HEAD_PROJECTION_SCHEMA_VERSION {
            return Err(MejepaInferError::HeadProjectionSchemaMismatch {
                expected: HEAD_PROJECTION_SCHEMA_VERSION.to_string(),
                actual: self.schema_version.to_string(),
            });
        }
        self.chunk_id.validate("head_projection.chunk_id")?;
        if self.features.is_empty() {
            return Err(MejepaInferError::HeadProjectionMissingSlice {
                head: self.head.as_str().to_string(),
                slice: "features".to_string(),
                panel_id: panel_id_hex(self.panel_id),
                chunk_id: self.chunk_id.0.clone(),
            });
        }
        for feature in &self.features {
            if feature.name.trim().is_empty() {
                return Err(MejepaInferError::InvalidInput {
                    field: "head_projection.feature.name".to_string(),
                    detail: "feature name must be non-empty".to_string(),
                });
            }
            if !feature.value.is_finite() {
                return Err(MejepaInferError::NanDetected {
                    nan_source: format!("head_projection.{}.{}", self.head.as_str(), feature.name),
                    detail: format!("projected feature must be finite; got {}", feature.value),
                });
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct HeadProjectionInput<'a> {
    pub panel_id: PanelId,
    pub chunk_id: &'a ChunkId,
    pub signals: &'a DdaSignals,
    pub embedder_order: &'a [EmbedderId],
    pub scalar_features: &'a BTreeMap<String, f32>,
    pub schema: &'a str,
}

pub trait PerHeadProjection {
    fn project_head(
        &self,
        head: HeadId,
        input: HeadProjectionInput<'_>,
    ) -> Result<HeadProjection, MejepaInferError>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct DeterministicPerHeadProjection;

impl PerHeadProjection for DeterministicPerHeadProjection {
    fn project_head(
        &self,
        head: HeadId,
        input: HeadProjectionInput<'_>,
    ) -> Result<HeadProjection, MejepaInferError> {
        project_head(head, input)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HeadProjectionRegistryRecord {
    pub schema: String,
    pub schema_version: u32,
    pub head: HeadId,
    pub feature_count: usize,
    pub feature_schema_hash: String,
    pub features: Vec<HeadProjectionFeatureSpec>,
    pub source_signal_cf: String,
    pub source_of_truth_cf: String,
    pub created_at_unix_ms: i64,
}

impl HeadProjectionRegistryRecord {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        if self.schema != HEAD_PROJECTION_SCHEMA {
            return Err(MejepaInferError::HeadProjectionSchemaMismatch {
                expected: HEAD_PROJECTION_SCHEMA.to_string(),
                actual: self.schema.clone(),
            });
        }
        if self.schema_version != HEAD_PROJECTION_SCHEMA_VERSION {
            return Err(MejepaInferError::HeadProjectionSchemaMismatch {
                expected: HEAD_PROJECTION_SCHEMA_VERSION.to_string(),
                actual: self.schema_version.to_string(),
            });
        }
        if self.source_signal_cf != CF_MEJEPA_DDA_SIGNALS {
            return Err(MejepaInferError::InvalidInput {
                field: "head_projection_registry.source_signal_cf".to_string(),
                detail: format!("expected {CF_MEJEPA_DDA_SIGNALS}"),
            });
        }
        if self.source_of_truth_cf != CF_MEJEPA_HEAD_PROJECTION_REGISTRY {
            return Err(MejepaInferError::InvalidInput {
                field: "head_projection_registry.source_of_truth_cf".to_string(),
                detail: format!("expected {CF_MEJEPA_HEAD_PROJECTION_REGISTRY}"),
            });
        }
        if self.created_at_unix_ms <= 0 {
            return Err(MejepaInferError::InvalidInput {
                field: "head_projection_registry.created_at_unix_ms".to_string(),
                detail: "must be positive".to_string(),
            });
        }
        let spec = HeadProjectionSpec {
            schema: self.schema.clone(),
            schema_version: self.schema_version,
            head: self.head,
            features: self.features.clone(),
        };
        spec.validate()?;
        if self.feature_count != spec.features.len() {
            return Err(MejepaInferError::DimMismatch {
                expected: spec.features.len(),
                actual: self.feature_count,
                context: format!("{} registry feature_count mismatch", self.head.as_str()),
            });
        }
        let expected_hash = spec.feature_schema_hash()?;
        if self.feature_schema_hash != expected_hash {
            return Err(MejepaInferError::InvalidInput {
                field: "head_projection_registry.feature_schema_hash".to_string(),
                detail: format!(
                    "{} hash mismatch: expected {expected_hash}, got {}",
                    self.head.as_str(),
                    self.feature_schema_hash
                ),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HeadProjectionRegistryWriteSummary {
    pub rows_written: usize,
    pub byte_identical_readback: bool,
    pub source_of_truth_cf: String,
}

pub fn head_projection_spec(head: HeadId) -> HeadProjectionSpec {
    let features = match head {
        HeadId::Panel => vec![
            cosine(E_AST),
            cosine(E_DIFF),
            cosine(E_ORACLE),
            pairwise(E_AST, E_DIFF),
        ],
        HeadId::Oracle => vec![cosine(E_ORACLE), cosine(E_TEST), cosine(E_REASONING)],
        HeadId::FailureMode => vec![
            cosine(E_AST),
            cosine(E_CFG),
            cosine(E_DATA_FLOW),
            cosine(E_TYPE_GRAPH),
            pairwise(E_TYPE_GRAPH, E_DATA_FLOW),
        ],
        HeadId::EdgeCase => vec![cosine(E_DATA_FLOW), scalar("coverage_delta")],
        HeadId::TechDebt => vec![
            scalar("complexity_delta"),
            scalar("length_delta"),
            scalar("nesting_delta"),
            cosine(E_AST),
        ],
        HeadId::Perf => vec![
            cosine(E_CFG),
            scalar("runtime_latency_delta"),
            scalar("runtime_vram_delta"),
            scalar("runtime_cost_delta"),
        ],
        HeadId::Security => vec![cosine(E_STATIC_ANALYSIS), scalar("sast_findings")],
        HeadId::Accuracy => vec![cosine(E_TEST), scalar("metric_delta")],
        HeadId::Cost => vec![scalar("cost_ci_minutes_delta"), scalar("cost_tokens_delta")],
        HeadId::Reasoning => vec![
            pairwise(E_REASONING, E_DIFF),
            pairwise(E_REASONING, E_ORACLE),
        ],
    };
    HeadProjectionSpec {
        schema: HEAD_PROJECTION_SCHEMA.to_string(),
        schema_version: HEAD_PROJECTION_SCHEMA_VERSION,
        head,
        features,
    }
}

pub fn head_projection_registry_records(
    created_at_unix_ms: i64,
) -> Result<Vec<HeadProjectionRegistryRecord>, MejepaInferError> {
    HeadId::ALL
        .into_iter()
        .map(|head| {
            let spec = head_projection_spec(head);
            let record = HeadProjectionRegistryRecord {
                schema: HEAD_PROJECTION_SCHEMA.to_string(),
                schema_version: HEAD_PROJECTION_SCHEMA_VERSION,
                head,
                feature_count: spec.features.len(),
                feature_schema_hash: spec.feature_schema_hash()?,
                features: spec.features,
                source_signal_cf: CF_MEJEPA_DDA_SIGNALS.to_string(),
                source_of_truth_cf: CF_MEJEPA_HEAD_PROJECTION_REGISTRY.to_string(),
                created_at_unix_ms,
            };
            record.validate()?;
            Ok(record)
        })
        .collect()
}

pub fn project_head(
    head: HeadId,
    input: HeadProjectionInput<'_>,
) -> Result<HeadProjection, MejepaInferError> {
    validate_input_shape(head, input)?;
    let spec = head_projection_spec(head);
    spec.validate()?;
    let mut values = Vec::with_capacity(spec.features.len());
    for feature in &spec.features {
        let value = match &feature.source {
            HeadProjectionFeatureSource::PerEmbedderCosine { embedder_id } => {
                let idx = embedder_index(input, head, &embedder_id.0)?;
                *input
                    .signals
                    .per_embedder_cosine
                    .get(idx)
                    .ok_or_else(|| missing_slice(input, head, &feature.name))?
            }
            HeadProjectionFeatureSource::PairwiseCosine { left, right } => {
                let left_idx = embedder_index(input, head, &left.0)?;
                let right_idx = embedder_index(input, head, &right.0)?;
                let pair_idx =
                    upper_triangle_index(left_idx, right_idx, input.embedder_order.len())
                        .ok_or_else(|| missing_slice(input, head, &feature.name))?;
                *input
                    .signals
                    .pairwise_cosine_upper
                    .get(pair_idx)
                    .ok_or_else(|| missing_slice(input, head, &feature.name))?
            }
            HeadProjectionFeatureSource::Scalar { name } => *input
                .scalar_features
                .get(name)
                .ok_or_else(|| missing_slice(input, head, &feature.name))?,
        };
        if !value.is_finite() {
            return Err(MejepaInferError::NanDetected {
                nan_source: format!("head_projection.{}.{}", head.as_str(), feature.name),
                detail: format!("projected feature must be finite; got {value}"),
            });
        }
        values.push(HeadProjectionFeatureValue {
            name: feature.name.clone(),
            value,
        });
    }
    let projection = HeadProjection {
        schema: HEAD_PROJECTION_SCHEMA.to_string(),
        schema_version: HEAD_PROJECTION_SCHEMA_VERSION,
        panel_id: input.panel_id,
        chunk_id: input.chunk_id.clone(),
        head,
        features: values,
    };
    projection.validate()?;
    Ok(projection)
}

pub fn project_per_head_dda_rows(
    panel_id: PanelId,
    rows: &[(ChunkId, DdaSignals)],
    embedder_order: &[EmbedderId],
    scalars_by_chunk: &BTreeMap<ChunkId, BTreeMap<String, f32>>,
) -> Result<BTreeMap<HeadId, Vec<HeadProjection>>, MejepaInferError> {
    if rows.is_empty() {
        return Err(MejepaInferError::HeadProjectionNoDda {
            panel_id: panel_id_hex(panel_id),
        });
    }
    let projector = DeterministicPerHeadProjection;
    let mut out = BTreeMap::<HeadId, Vec<HeadProjection>>::new();
    for (chunk_id, signals) in rows {
        let scalars = scalars_by_chunk.get(chunk_id).ok_or_else(|| {
            MejepaInferError::HeadProjectionMissingSlice {
                head: "all".to_string(),
                slice: "scalar_features".to_string(),
                panel_id: panel_id_hex(panel_id),
                chunk_id: chunk_id.0.clone(),
            }
        })?;
        for head in HeadId::ALL {
            let projection = projector.project_head(
                head,
                HeadProjectionInput {
                    panel_id,
                    chunk_id,
                    signals,
                    embedder_order,
                    scalar_features: scalars,
                    schema: HEAD_PROJECTION_SCHEMA,
                },
            )?;
            out.entry(head).or_default().push(projection);
        }
    }
    Ok(out)
}

pub fn write_head_projection_registry_sync_readback(
    db: &DB,
    records: &[HeadProjectionRegistryRecord],
) -> Result<HeadProjectionRegistryWriteSummary, MejepaInferError> {
    let cf = cf(db, CF_MEJEPA_HEAD_PROJECTION_REGISTRY)?;
    let mut opts = WriteOptions::default();
    opts.set_sync(true);
    let mut encoded = Vec::with_capacity(records.len());
    for record in records {
        record.validate()?;
        let key = head_projection_registry_key(record.head);
        let value = serde_json::to_vec(record)?;
        db.put_cf_opt(cf, &key, &value, &opts)?;
        encoded.push((key, value));
    }
    db.flush_cf(cf)?;
    for (key, value) in &encoded {
        let readback = db
            .get_cf(cf, key)?
            .ok_or_else(|| MejepaInferError::InvalidInput {
                field: "head_projection_registry.readback".to_string(),
                detail: "sync write readback returned no row".to_string(),
            })?;
        if &readback != value {
            return Err(MejepaInferError::InvalidInput {
                field: "head_projection_registry.readback".to_string(),
                detail: "read-after-write bytes differ from encoded input".to_string(),
            });
        }
    }
    Ok(HeadProjectionRegistryWriteSummary {
        rows_written: records.len(),
        byte_identical_readback: true,
        source_of_truth_cf: CF_MEJEPA_HEAD_PROJECTION_REGISTRY.to_string(),
    })
}

pub fn read_head_projection_registry(
    db: &DB,
) -> Result<Vec<HeadProjectionRegistryRecord>, MejepaInferError> {
    let cf = cf(db, CF_MEJEPA_HEAD_PROJECTION_REGISTRY)?;
    let mut rows = Vec::new();
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let (_key, value) = item?;
        let row: HeadProjectionRegistryRecord = serde_json::from_slice(&value)?;
        row.validate()?;
        rows.push(row);
    }
    rows.sort_by_key(|row| row.head);
    Ok(rows)
}

pub fn count_head_projection_registry(db: &DB) -> Result<usize, MejepaInferError> {
    let cf = cf(db, CF_MEJEPA_HEAD_PROJECTION_REGISTRY)?;
    let mut count = 0usize;
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let _ = item?;
        count += 1;
    }
    Ok(count)
}

pub fn head_projection_registry_key(head: HeadId) -> Vec<u8> {
    format!("{HEAD_PROJECTION_SCHEMA}/{}", head.as_str()).into_bytes()
}

pub fn upper_triangle_index(i: usize, j: usize, n: usize) -> Option<usize> {
    if i == j || i >= n || j >= n {
        return None;
    }
    let (a, b) = if i < j { (i, j) } else { (j, i) };
    Some(a * n - (a * (a + 1) / 2) + (b - a - 1))
}

pub fn panel_id_hex(panel_id: PanelId) -> String {
    hex::encode(panel_id.0)
}

fn validate_input_shape(
    head: HeadId,
    input: HeadProjectionInput<'_>,
) -> Result<(), MejepaInferError> {
    if input.schema != HEAD_PROJECTION_SCHEMA {
        return Err(MejepaInferError::HeadProjectionSchemaMismatch {
            expected: HEAD_PROJECTION_SCHEMA.to_string(),
            actual: input.schema.to_string(),
        });
    }
    input.chunk_id.validate("head_projection_input.chunk_id")?;
    input.signals.validate()?;
    if input.signals.embedder_count() == 0 {
        return Err(MejepaInferError::HeadProjectionNoDda {
            panel_id: panel_id_hex(input.panel_id),
        });
    }
    if input.embedder_order.len() != input.signals.embedder_count() {
        return Err(MejepaInferError::DimMismatch {
            expected: input.signals.embedder_count(),
            actual: input.embedder_order.len(),
            context: format!(
                "{} embedder order does not match DDA vector width",
                head.as_str()
            ),
        });
    }
    for (idx, embedder) in input.embedder_order.iter().enumerate() {
        embedder.validate(&format!("head_projection_input.embedder_order[{idx}]"))?;
    }
    for (name, value) in input.scalar_features {
        validate_head_projection_scalar_feature_name(name)?;
        if !value.is_finite() {
            return Err(MejepaInferError::NanDetected {
                nan_source: format!("head_projection_input.scalar_features.{name}"),
                detail: format!("scalar feature must be finite; got {value}"),
            });
        }
    }
    Ok(())
}

pub fn validate_head_projection_scalar_feature_name(name: &str) -> Result<(), MejepaInferError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(MejepaInferError::InvalidInput {
            field: "head_projection_input.scalar_features".to_string(),
            detail: "scalar feature names must be non-empty".to_string(),
        });
    }
    let lower = trimmed.to_ascii_lowercase();
    if lower.starts_with(HEAD_PROJECTION_GEOMETRY_SCALAR_PREFIX) {
        if HEAD_PROJECTION_ALLOWED_GEOMETRY_SCALARS.contains(&trimmed) {
            return Ok(());
        }
        return Err(MejepaInferError::InvalidInput {
            field: "head_projection_input.scalar_features.geometry_scalar_contract".to_string(),
            detail: format!(
                "undeclared geometry scalar '{trimmed}' is not in {HEAD_PROJECTION_GEOMETRY_SCALAR_SCHEMA}"
            ),
        });
    }
    if is_raw_geometry_identifier_name(&lower) {
        return Err(MejepaInferError::InvalidInput {
            field: "head_projection_input.scalar_features.geometry_scalar_contract".to_string(),
            detail: format!(
                "raw high-cardinality geometry identifier '{trimmed}' is not a scalar feature"
            ),
        });
    }
    if is_label_target_leak_name(&lower) {
        return Err(MejepaInferError::InvalidInput {
            field: "head_projection_input.scalar_features.geometry_scalar_contract".to_string(),
            detail: format!("target-side or partition-leaky label scalar '{trimmed}' is forbidden"),
        });
    }
    Ok(())
}

fn is_raw_geometry_identifier_name(lower: &str) -> bool {
    [
        "cluster_id",
        "region_id",
        "label_id",
        "geo_label:",
        "microcell_id",
        "singleton_label_id",
        "raw_cluster",
        "raw_region",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

fn is_label_target_leak_name(lower: &str) -> bool {
    let looks_label_derived = lower.contains("geometry")
        || lower.contains("label")
        || lower.contains("cluster")
        || lower.contains("region");
    looks_label_derived
        && [
            "oracle",
            "docker",
            "test_outcome",
            "holdout",
            "target_side",
            "ground_truth",
            "partition",
        ]
        .iter()
        .any(|needle| lower.contains(needle))
}

fn embedder_index(
    input: HeadProjectionInput<'_>,
    head: HeadId,
    embedder_id: &str,
) -> Result<usize, MejepaInferError> {
    input
        .embedder_order
        .iter()
        .position(|candidate| candidate.0 == embedder_id)
        .ok_or_else(|| MejepaInferError::HeadProjectionMissingSlice {
            head: head.as_str().to_string(),
            slice: format!("cosine_to_centroid:{embedder_id}"),
            panel_id: panel_id_hex(input.panel_id),
            chunk_id: input.chunk_id.0.clone(),
        })
}

fn missing_slice(input: HeadProjectionInput<'_>, head: HeadId, slice: &str) -> MejepaInferError {
    MejepaInferError::HeadProjectionMissingSlice {
        head: head.as_str().to_string(),
        slice: slice.to_string(),
        panel_id: panel_id_hex(input.panel_id),
        chunk_id: input.chunk_id.0.clone(),
    }
}

fn cosine(embedder_id: &str) -> HeadProjectionFeatureSpec {
    HeadProjectionFeatureSpec {
        name: format!("cosine_to_centroid:{embedder_id}"),
        source: HeadProjectionFeatureSource::PerEmbedderCosine {
            embedder_id: EmbedderId(embedder_id.to_string()),
        },
    }
}

fn pairwise(left: &str, right: &str) -> HeadProjectionFeatureSpec {
    HeadProjectionFeatureSpec {
        name: format!("pairwise_cosine:{left}:{right}"),
        source: HeadProjectionFeatureSource::PairwiseCosine {
            left: EmbedderId(left.to_string()),
            right: EmbedderId(right.to_string()),
        },
    }
}

fn scalar(name: &str) -> HeadProjectionFeatureSpec {
    HeadProjectionFeatureSpec {
        name: format!("scalar:{name}"),
        source: HeadProjectionFeatureSource::Scalar {
            name: name.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upper_triangle_index_matches_row_major_strict_upper_triangle() {
        let n = 5;
        let expected = [
            (0, 1, 0),
            (0, 2, 1),
            (0, 3, 2),
            (0, 4, 3),
            (1, 2, 4),
            (1, 3, 5),
            (1, 4, 6),
            (2, 3, 7),
            (2, 4, 8),
            (3, 4, 9),
        ];
        for (left, right, idx) in expected {
            assert_eq!(upper_triangle_index(left, right, n), Some(idx));
            assert_eq!(upper_triangle_index(right, left, n), Some(idx));
        }
        assert_eq!(upper_triangle_index(2, 2, n), None);
        assert_eq!(upper_triangle_index(0, 5, n), None);
    }

    #[test]
    fn canonical_head_specs_are_not_identical() {
        let oracle = head_projection_spec(HeadId::Oracle);
        let failure = head_projection_spec(HeadId::FailureMode);
        assert_eq!(oracle.features.len(), 3);
        assert_eq!(failure.features.len(), 5);
        assert_ne!(oracle.features, failure.features);
    }

    #[test]
    fn missing_reasoning_slice_fails_closed() {
        let panel_id = PanelId([7u8; 32]);
        let chunk_id = ChunkId("chunk-missing-reasoning".to_string());
        let embedder_order = vec![
            EmbedderId(E_ORACLE.to_string()),
            EmbedderId(E_TEST.to_string()),
            EmbedderId(E_AST.to_string()),
        ];
        let signals = DdaSignals {
            per_embedder_cosine: vec![0.1, 0.2, 0.3],
            pairwise_cosine_upper: vec![0.4, 0.5, 0.6],
            pairwise_mi_upper: vec![0.1, 0.1, 0.1],
            blind_spot_z_scores: vec![0.0, 0.0, 0.0],
        };
        let err = project_head(
            HeadId::Oracle,
            HeadProjectionInput {
                panel_id,
                chunk_id: &chunk_id,
                signals: &signals,
                embedder_order: &embedder_order,
                scalar_features: &BTreeMap::new(),
                schema: HEAD_PROJECTION_SCHEMA,
            },
        )
        .expect_err("missing E_Reasoning must fail closed");
        assert_eq!(err.code(), "HEAD_PROJECTION_MISSING_SLICE");
    }

    #[test]
    fn declared_geometry_scalar_names_are_accepted() {
        for name in HEAD_PROJECTION_ALLOWED_GEOMETRY_SCALARS {
            validate_head_projection_scalar_feature_name(name).expect("declared scalar");
        }
    }

    #[test]
    fn raw_geometry_scalar_identifiers_fail_closed() {
        let err = validate_head_projection_scalar_feature_name("geometry:cluster_id:e7:42")
            .expect_err("raw cluster id must not be scalarized");
        assert_eq!(err.code(), "MEJEPA_INFER_INVALID_INPUT");

        let err = validate_head_projection_scalar_feature_name("region_id")
            .expect_err("raw region id must not be scalarized");
        assert_eq!(err.code(), "MEJEPA_INFER_INVALID_INPUT");
    }

    #[test]
    fn target_leaky_label_scalar_names_fail_closed() {
        let err = validate_head_projection_scalar_feature_name("label_quality_oracle_agreement")
            .expect_err("oracle-derived label scalar must fail closed");
        assert_eq!(err.code(), "MEJEPA_INFER_INVALID_INPUT");

        let err = validate_head_projection_scalar_feature_name("geometry:holdout_selected_density")
            .expect_err("holdout-selected geometry scalar must fail closed");
        assert_eq!(err.code(), "MEJEPA_INFER_INVALID_INPUT");
    }

    #[test]
    fn declared_geometry_scalar_extras_do_not_break_legacy_heads() {
        let panel_id = PanelId([3u8; 32]);
        let chunk_id = ChunkId("chunk-geometry-scalar-contract".to_string());
        let embedder_order = vec![EmbedderId(E_AST.to_string())];
        let signals = DdaSignals {
            per_embedder_cosine: vec![0.5],
            pairwise_cosine_upper: vec![],
            pairwise_mi_upper: vec![],
            blind_spot_z_scores: vec![],
        };
        let mut scalars = BTreeMap::new();
        scalars.insert("complexity_delta".to_string(), 1.0);
        scalars.insert("length_delta".to_string(), 2.0);
        scalars.insert("nesting_delta".to_string(), 3.0);
        scalars.insert(
            "geometry:label_quality_signal_minus_noise".to_string(),
            0.25,
        );

        let projection = project_head(
            HeadId::TechDebt,
            HeadProjectionInput {
                panel_id,
                chunk_id: &chunk_id,
                signals: &signals,
                embedder_order: &embedder_order,
                scalar_features: &scalars,
                schema: HEAD_PROJECTION_SCHEMA,
            },
        )
        .expect("declared geometry scalar extras should validate");
        assert_eq!(projection.features.len(), 4);
    }
}
