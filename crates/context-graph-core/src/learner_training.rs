//! Matrix-shaped learner training datasets.
//!
//! These records are compiled from the learner and Learning-as-UTL RocksDB
//! column families into row-major `f32` matrices so downstream trainers can
//! consume stable tensors without re-scanning heterogeneous CF payloads.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::error::{CoreError, CoreResult};
use crate::learning::LearningEvent;
use crate::training::NUM_CROSS_CORRELATIONS;
use crate::types::fingerprint::NUM_EMBEDDERS;

/// Current on-disk version byte for learner training datasets.
pub const LEARNER_TRAINING_DATASET_VERSION: u8 = 1;

/// Hard safety bound for one stored matrix shard.
pub const MAX_LEARNER_TRAINING_ROWS: usize = 1_000_000;

/// Feature vectors are intentionally compact derived views, not raw model
/// embeddings.
pub const MAX_LEARNER_TRAINING_COLS: usize = 4096;

/// Offline task family represented by one matrix shard.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum LearnerTrainingTask {
    RewardModel,
    Reranker,
    EmbedderContrastive,
    DiagnosticClassifier,
    Scheduler,
    PersonalPhysiology,
}

impl LearnerTrainingTask {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RewardModel => "reward_model",
            Self::Reranker => "reranker",
            Self::EmbedderContrastive => "embedder_contrastive",
            Self::DiagnosticClassifier => "diagnostic_classifier",
            Self::Scheduler => "scheduler",
            Self::PersonalPhysiology => "personal_physiology",
        }
    }

    pub fn parse(value: &str) -> CoreResult<Self> {
        match value {
            "reward_model" => Ok(Self::RewardModel),
            "reranker" => Ok(Self::Reranker),
            "embedder_contrastive" => Ok(Self::EmbedderContrastive),
            "diagnostic_classifier" => Ok(Self::DiagnosticClassifier),
            "scheduler" => Ok(Self::Scheduler),
            "personal_physiology" => Ok(Self::PersonalPhysiology),
            other => Err(CoreError::ValidationError {
                field: "learner_training.task".into(),
                message: format!("unknown learner training task: {other}"),
            }),
        }
    }
}

/// Per-row metadata. The feature tensor itself lives in
/// `LearnerTrainingDataset::row_major`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LearnerTrainingRow {
    pub row_id: Uuid,
    pub source_cf: String,
    pub source_key: String,
    pub event_id: Option<Uuid>,
    pub learner_id: Option<Uuid>,
    pub session_ts: Option<u64>,
    pub label_scalar: Option<f32>,
    pub label_class: Option<String>,
    pub split_key: String,
    pub provenance_sha256: String,
}

impl LearnerTrainingRow {
    pub fn validate(&self) -> CoreResult<()> {
        validate_label(&self.source_cf, "learner_training.row.source_cf")?;
        validate_label(&self.source_key, "learner_training.row.source_key")?;
        validate_label(&self.split_key, "learner_training.row.split_key")?;
        if let Some(label) = self.label_scalar {
            if !label.is_finite() {
                return Err(CoreError::ValidationError {
                    field: "learner_training.row.label_scalar".into(),
                    message: "must be finite".into(),
                });
            }
        }
        if let Some(label) = self.label_class.as_ref() {
            validate_label(label, "learner_training.row.label_class")?;
        }
        validate_sha256(
            &self.provenance_sha256,
            "learner_training.row.provenance_sha256",
        )
    }
}

/// A matrix shard compiled from one or more learner CFs.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LearnerTrainingDataset {
    pub dataset_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub task: LearnerTrainingTask,
    pub feature_schema: Vec<String>,
    pub label_schema: Vec<String>,
    pub rows_len: u32,
    pub cols_len: u32,
    pub rows: Vec<LearnerTrainingRow>,
    pub row_major: Vec<f32>,
    pub source_counts: BTreeMap<String, u64>,
    pub filters: BTreeMap<String, String>,
    pub row_major_sha256: String,
    pub provenance_manifest_sha256: String,
}

impl LearnerTrainingDataset {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        dataset_id: Uuid,
        task: LearnerTrainingTask,
        feature_schema: Vec<String>,
        label_schema: Vec<String>,
        rows: Vec<LearnerTrainingRow>,
        row_major: Vec<f32>,
        source_counts: BTreeMap<String, u64>,
        filters: BTreeMap<String, String>,
    ) -> CoreResult<Self> {
        let row_major_sha256 = sha256_row_major(&row_major);
        let provenance_manifest_sha256 =
            sha256_manifest(&task, &feature_schema, &label_schema, &rows, &source_counts);
        let dataset = Self {
            dataset_id,
            created_at: Utc::now(),
            task,
            rows_len: rows.len() as u32,
            cols_len: feature_schema.len() as u32,
            feature_schema,
            label_schema,
            rows,
            row_major,
            source_counts,
            filters,
            row_major_sha256,
            provenance_manifest_sha256,
        };
        dataset.validate()?;
        Ok(dataset)
    }

    pub fn validate(&self) -> CoreResult<()> {
        if self.feature_schema.is_empty() {
            return Err(CoreError::ValidationError {
                field: "learner_training.feature_schema".into(),
                message: "must not be empty".into(),
            });
        }
        if self.feature_schema.len() > MAX_LEARNER_TRAINING_COLS {
            return Err(CoreError::ValidationError {
                field: "learner_training.feature_schema".into(),
                message: format!(
                    "len {} exceeds max {}",
                    self.feature_schema.len(),
                    MAX_LEARNER_TRAINING_COLS
                ),
            });
        }
        if self.rows.len() > MAX_LEARNER_TRAINING_ROWS {
            return Err(CoreError::ValidationError {
                field: "learner_training.rows".into(),
                message: format!(
                    "len {} exceeds max {}",
                    self.rows.len(),
                    MAX_LEARNER_TRAINING_ROWS
                ),
            });
        }
        if self.rows_len as usize != self.rows.len() {
            return Err(CoreError::ValidationError {
                field: "learner_training.rows_len".into(),
                message: "must match rows.len()".into(),
            });
        }
        if self.cols_len as usize != self.feature_schema.len() {
            return Err(CoreError::ValidationError {
                field: "learner_training.cols_len".into(),
                message: "must match feature_schema.len()".into(),
            });
        }
        let expected = self.rows.len() * self.feature_schema.len();
        if self.row_major.len() != expected {
            return Err(CoreError::ValidationError {
                field: "learner_training.row_major".into(),
                message: format!(
                    "len {} does not match rows*cols {}",
                    self.row_major.len(),
                    expected
                ),
            });
        }
        for name in &self.feature_schema {
            validate_label(name, "learner_training.feature_schema")?;
        }
        for name in &self.label_schema {
            validate_label(name, "learner_training.label_schema")?;
        }
        for value in &self.row_major {
            if !value.is_finite() {
                return Err(CoreError::ValidationError {
                    field: "learner_training.row_major".into(),
                    message: "all values must be finite".into(),
                });
            }
        }
        for row in &self.rows {
            row.validate()?;
        }
        validate_sha256(&self.row_major_sha256, "learner_training.row_major_sha256")?;
        validate_sha256(
            &self.provenance_manifest_sha256,
            "learner_training.provenance_manifest_sha256",
        )?;
        let actual = sha256_row_major(&self.row_major);
        if actual != self.row_major_sha256 {
            return Err(CoreError::ValidationError {
                field: "learner_training.row_major_sha256".into(),
                message: "does not match row_major payload".into(),
            });
        }
        Ok(())
    }
}

pub fn sha256_row_major(values: &[f32]) -> String {
    let mut hasher = Sha256::new();
    for value in values {
        hasher.update(value.to_le_bytes());
    }
    hex_lower(&hasher.finalize())
}

/// Canonical feature schema for Learning-as-UTL event tensors.
///
/// This is the shared contract used by MCP matrix export, CLI JSONL export,
/// and downstream DPO/RL trainers. Keep changes append-only and versioned
/// through `LEARNER_TRAINING_DATASET_VERSION`.
pub fn learning_event_feature_schema() -> Vec<String> {
    let mut schema = Vec::new();
    for i in 1..=NUM_EMBEDDERS {
        schema.push(format!("before_topic_e{i}"));
    }
    for i in 0..NUM_CROSS_CORRELATIONS {
        schema.push(format!("before_cross_correlation_{i:02}"));
    }
    for i in 1..=NUM_EMBEDDERS {
        schema.push(format!("before_embedder_score_e{i}"));
    }
    for i in 1..=NUM_EMBEDDERS {
        schema.push(format!("delta_topic_e{i}"));
    }
    schema.extend([
        "before_retrieval_rank_norm".into(),
        "after_retrieval_rank_norm".into(),
        "before_contradiction_pressure".into(),
        "after_contradiction_pressure".into(),
        "before_integration_confidence".into(),
        "after_integration_confidence".into(),
        "before_stability_score".into(),
        "after_stability_score".into(),
        "surprise_score".into(),
        "coherence_delta".into(),
        "contradiction_delta".into(),
        "consolidation_readiness".into(),
        "transfer_score".into(),
        "multi_utl_score".into(),
        "embedder_disagreement".into(),
        "retrieval_rank_shift".into(),
    ]);
    schema
}

pub fn learning_event_label_schema() -> Vec<String> {
    vec![
        "utility_delta".into(),
        "outcome_label".into(),
        "correction_required".into(),
        "reuse_observed".into(),
    ]
}

/// Convert a persisted Learning-as-UTL event into the canonical row-major
/// tensor row. Validation is strict: malformed event state returns an error
/// instead of emitting a partially-populated tensor.
pub fn learning_event_feature_vector(event: &LearningEvent) -> CoreResult<Vec<f32>> {
    event.validate()?;

    let mut features = Vec::with_capacity(learning_event_feature_schema().len());
    features.extend_from_slice(&event.before.topic_profile);
    features.extend_from_slice(&event.before.cross_correlations);
    features.extend_from_slice(&event.before.embedder_scores);
    for i in 0..NUM_EMBEDDERS {
        features.push(event.after.topic_profile[i] - event.before.topic_profile[i]);
    }
    features.push(rank_norm(event.before.retrieval_rank));
    features.push(rank_norm(event.after.retrieval_rank));
    features.push(event.before.contradiction_pressure);
    features.push(event.after.contradiction_pressure);
    features.push(event.before.integration_confidence);
    features.push(event.after.integration_confidence);
    features.push(event.before.stability_score);
    features.push(event.after.stability_score);
    features.push(event.features.surprise_score);
    features.push(event.features.coherence_delta);
    features.push(event.features.contradiction_delta);
    features.push(event.features.consolidation_readiness);
    features.push(event.features.transfer_score);
    features.push(event.features.multi_utl_score);
    features.push(event.features.embedder_disagreement);
    features.push(event.features.retrieval_rank_shift);

    let expected = learning_event_feature_schema().len();
    validate_feature_tensor(&features, expected, "learning_event_feature_vector")?;
    Ok(features)
}

pub fn validate_feature_tensor(values: &[f32], expected_len: usize, field: &str) -> CoreResult<()> {
    if values.len() != expected_len {
        return Err(CoreError::DimensionMismatch {
            expected: expected_len,
            actual: values.len(),
        });
    }
    for (idx, value) in values.iter().enumerate() {
        if !value.is_finite() {
            return Err(CoreError::ValidationError {
                field: format!("{field}[{idx}]"),
                message: "must be finite".into(),
            });
        }
    }
    Ok(())
}

fn rank_norm(rank: Option<u32>) -> f32 {
    rank.map(|rank| 1.0 / (1.0 + rank as f32)).unwrap_or(0.0)
}

fn sha256_manifest(
    task: &LearnerTrainingTask,
    feature_schema: &[String],
    label_schema: &[String],
    rows: &[LearnerTrainingRow],
    source_counts: &BTreeMap<String, u64>,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(task.as_str().as_bytes());
    for name in feature_schema {
        hasher.update(name.as_bytes());
        hasher.update([0]);
    }
    for name in label_schema {
        hasher.update(name.as_bytes());
        hasher.update([0]);
    }
    for row in rows {
        hasher.update(row.source_cf.as_bytes());
        hasher.update(row.source_key.as_bytes());
        hasher.update(row.provenance_sha256.as_bytes());
    }
    for (key, value) in source_counts {
        hasher.update(key.as_bytes());
        hasher.update(value.to_le_bytes());
    }
    hex_lower(&hasher.finalize())
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

fn validate_label(value: &str, field: &str) -> CoreResult<()> {
    if value.is_empty() || value.len() > 256 {
        return Err(CoreError::ValidationError {
            field: field.into(),
            message: "must be non-empty and <= 256 bytes".into(),
        });
    }
    Ok(())
}

fn validate_sha256(value: &str, field: &str) -> CoreResult<()> {
    if value.len() != 64 || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(CoreError::ValidationError {
            field: field.into(),
            message: "must be a 64-character hex SHA-256".into(),
        });
    }
    Ok(())
}
