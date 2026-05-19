// TASK-PY-G-050: durable Q4 reasoning-class labels from observed agent prose.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{InstrumentError, InstrumentResult};

const Q4_REASONING_SCHEMA_VERSION: u32 = 1;
const MAX_AGENT_PROSE_BYTES: usize = 2_000_000;
const MAX_LABELS: usize = 100_000;
const FEATURE_DIM: usize = 12;
pub const Q4_REASONING_MIN_HARVEST_EXAMPLES: usize = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Q4ReasoningOutcome {
    Pass,
    Fail,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Q4ReasoningPredictionVerdict {
    Pass,
    Fail,
    OutOfDistribution,
    Abstain,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Q4ReasoningClass {
    Unknown,
    None,
    CodeOnly,
    Unsupported,
    Hedging,
    Overclaiming,
    Calibrated,
    Apologetic,
    ConfidentCorrect,
    ConfidentWrong,
}

impl Q4ReasoningClass {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::None => "none",
            Self::CodeOnly => "code_only",
            Self::Unsupported => "unsupported",
            Self::Hedging => "hedging",
            Self::Overclaiming => "overclaiming",
            Self::Calibrated => "calibrated",
            Self::Apologetic => "apologetic",
            Self::ConfidentCorrect => "confident_correct",
            Self::ConfidentWrong => "confident_wrong",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Q4ReasoningSource {
    pub corpus_row_id: String,
    pub chunk_id: String,
    pub session_id: String,
    pub agent_prose: String,
    pub oracle_outcome: Q4ReasoningOutcome,
    pub prediction_verdict: Q4ReasoningPredictionVerdict,
    pub pairwise_cosine_reasoning_diff: f64,
    pub pairwise_cosine_reasoning_oracle: f64,
    pub e17_affect_score: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Q4ReasoningFeatures {
    pub hedge_terms: u32,
    pub apology_terms: u32,
    pub confidence_terms: u32,
    pub success_claim_terms: u32,
    pub failure_ack_terms: u32,
    pub natural_language_words: u32,
    pub code_like_lines: u32,
    pub non_ascii_ratio: f64,
    pub pairwise_cosine_reasoning_diff: f64,
    pub pairwise_cosine_reasoning_oracle: f64,
    pub e17_affect_score: f64,
    pub oracle_pass: bool,
}

impl Q4ReasoningFeatures {
    pub fn vector(&self) -> [f64; FEATURE_DIM] {
        [
            self.hedge_terms as f64,
            self.apology_terms as f64,
            self.confidence_terms as f64,
            self.success_claim_terms as f64,
            self.failure_ack_terms as f64,
            self.natural_language_words as f64 / 100.0,
            self.code_like_lines as f64 / 20.0,
            self.non_ascii_ratio,
            self.pairwise_cosine_reasoning_diff,
            self.pairwise_cosine_reasoning_oracle,
            self.e17_affect_score,
            if self.oracle_pass { 1.0 } else { 0.0 },
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Q4ReasoningLabel {
    pub corpus_row_id: String,
    pub chunk_id: String,
    pub session_id: String,
    pub class: Q4ReasoningClass,
    pub confidence: f64,
    pub reason: String,
    pub oracle_outcome: Q4ReasoningOutcome,
    pub prediction_verdict: Q4ReasoningPredictionVerdict,
    pub features: Q4ReasoningFeatures,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Q4ReasoningQuarantine {
    pub corpus_row_id: String,
    pub chunk_id: String,
    pub session_id: String,
    pub reason_code: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "record_kind", content = "record", rename_all = "snake_case")]
pub enum Q4ReasoningSignalRecord {
    Label(Q4ReasoningLabel),
    Quarantine(Q4ReasoningQuarantine),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PersistedQ4ReasoningSignal {
    pub schema_version: u32,
    pub signal: Q4ReasoningSignalRecord,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Q4ReasoningCalibrationRow {
    pub head: String,
    pub class: Q4ReasoningClass,
    pub support: u64,
    pub precision: f64,
    pub recall: f64,
    pub f1: f64,
    pub checkpoint_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PersistedQ4ReasoningCalibration {
    pub schema_version: u32,
    pub row: Q4ReasoningCalibrationRow,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Q4ReasoningExtraction {
    pub labels: Vec<Q4ReasoningLabel>,
    pub quarantines: Vec<Q4ReasoningQuarantine>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Q4ReasoningHeadStatus {
    Trained,
    UnknownInsufficientHarvest,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Q4ReasoningHeadCheckpoint {
    pub schema_version: u32,
    pub status: Q4ReasoningHeadStatus,
    pub trained_rows: u64,
    pub holdout_rows: u64,
    pub min_harvest_examples: u64,
    pub feature_dim: usize,
    pub centroids: BTreeMap<Q4ReasoningClass, Vec<f64>>,
    pub class_metrics: Vec<Q4ReasoningCalibrationRow>,
    pub fallback_class: Q4ReasoningClass,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Q4ReasoningRawOutputPaths {
    pub harvest_id: String,
    pub root: PathBuf,
    pub harvest_json: PathBuf,
}

pub fn extract_q4_reasoning_labels(
    sources: &[Q4ReasoningSource],
) -> InstrumentResult<Q4ReasoningExtraction> {
    if sources.len() > MAX_LABELS {
        return invalid(
            "q4_reasoning.sources",
            format!(
                "Q4 reasoning source count {} exceeds {MAX_LABELS}",
                sources.len()
            ),
            "shard large reasoning harvests before extracting labels",
        );
    }
    let mut labels = Vec::with_capacity(sources.len());
    let quarantines = Vec::new();
    for source in sources {
        validate_source(source)?;
        let label = classify_source(source)?;
        validate_label(&label, source)?;
        labels.push(label);
    }
    Ok(Q4ReasoningExtraction {
        labels,
        quarantines,
    })
}

pub fn train_q4_reasoning_head(
    labels: &[Q4ReasoningLabel],
) -> InstrumentResult<Q4ReasoningHeadCheckpoint> {
    validate_labels(labels)?;
    if labels.len() < Q4_REASONING_MIN_HARVEST_EXAMPLES {
        return Ok(Q4ReasoningHeadCheckpoint {
            schema_version: Q4_REASONING_SCHEMA_VERSION,
            status: Q4ReasoningHeadStatus::UnknownInsufficientHarvest,
            trained_rows: labels.len() as u64,
            holdout_rows: 0,
            min_harvest_examples: Q4_REASONING_MIN_HARVEST_EXAMPLES as u64,
            feature_dim: FEATURE_DIM,
            centroids: BTreeMap::new(),
            class_metrics: Vec::new(),
            fallback_class: Q4ReasoningClass::Unknown,
        });
    }

    let mut sums: BTreeMap<Q4ReasoningClass, [f64; FEATURE_DIM]> = BTreeMap::new();
    let mut counts: BTreeMap<Q4ReasoningClass, u64> = BTreeMap::new();
    let mut train_rows = 0_u64;
    let mut holdout = Vec::new();
    for (idx, label) in labels.iter().enumerate() {
        if idx % 5 == 0 {
            holdout.push(label);
            continue;
        }
        let vector = label.features.vector();
        let sum = sums.entry(label.class).or_insert([0.0; FEATURE_DIM]);
        for i in 0..FEATURE_DIM {
            sum[i] += vector[i];
        }
        *counts.entry(label.class).or_default() += 1;
        train_rows += 1;
    }

    let mut centroids = BTreeMap::new();
    for (class, sum) in sums {
        let count = counts.get(&class).copied().unwrap_or(1) as f64;
        centroids.insert(class, sum.into_iter().map(|value| value / count).collect());
    }
    let metrics = evaluate_holdout(&centroids, &holdout, "pending")?;
    let checkpoint = Q4ReasoningHeadCheckpoint {
        schema_version: Q4_REASONING_SCHEMA_VERSION,
        status: Q4ReasoningHeadStatus::Trained,
        trained_rows: train_rows,
        holdout_rows: holdout.len() as u64,
        min_harvest_examples: Q4_REASONING_MIN_HARVEST_EXAMPLES as u64,
        feature_dim: FEATURE_DIM,
        centroids,
        class_metrics: metrics,
        fallback_class: Q4ReasoningClass::Unknown,
    };
    validate_checkpoint(&checkpoint)?;
    Ok(checkpoint)
}

pub fn write_q4_reasoning_raw_outputs(
    root: impl AsRef<Path>,
    harvest_id: &str,
    sources: &[Q4ReasoningSource],
    extraction: &Q4ReasoningExtraction,
    checkpoint: &Q4ReasoningHeadCheckpoint,
) -> InstrumentResult<Q4ReasoningRawOutputPaths> {
    validate_path_component("harvest_id", harvest_id)?;
    let root = root.as_ref().join(harvest_id);
    fs::create_dir_all(&root).map_err(|err| {
        InstrumentError::store(
            "create_dir_all",
            "python-q4-reasoning-labels-v1",
            err.to_string(),
            "ensure the aiwonder Q4 reasoning raw-output directory is writable",
        )
    })?;
    let paths = Q4ReasoningRawOutputPaths {
        harvest_id: harvest_id.to_string(),
        harvest_json: root.join("reasoning_harvest.json"),
        root,
    };
    let payload = serde_json::json!({
        "schemaVersion": Q4_REASONING_SCHEMA_VERSION,
        "sources": sources,
        "labels": extraction.labels,
        "quarantines": extraction.quarantines,
        "checkpoint": checkpoint,
    });
    let bytes = serde_json::to_vec_pretty(&payload).map_err(|err| {
        InstrumentError::store(
            "serialize",
            "python-q4-reasoning-labels-v1",
            err.to_string(),
            "keep Q4 reasoning raw output JSON-serializable",
        )
    })?;
    fs::write(&paths.harvest_json, bytes).map_err(|err| {
        InstrumentError::store(
            "write",
            "python-q4-reasoning-labels-v1",
            err.to_string(),
            "inspect aiwonder raw-output filesystem permissions and free space",
        )
    })?;
    Ok(paths)
}

pub fn attach_checkpoint_hash(
    mut checkpoint: Q4ReasoningHeadCheckpoint,
    checkpoint_sha256: &str,
) -> InstrumentResult<Q4ReasoningHeadCheckpoint> {
    validate_non_empty_single_line("checkpoint_sha256", checkpoint_sha256)?;
    for row in &mut checkpoint.class_metrics {
        row.checkpoint_sha256 = checkpoint_sha256.to_string();
    }
    validate_checkpoint(&checkpoint)?;
    Ok(checkpoint)
}

fn classify_source(source: &Q4ReasoningSource) -> InstrumentResult<Q4ReasoningLabel> {
    let features = extract_features(source)?;
    let text = source.agent_prose.trim();
    let lower = text.to_ascii_lowercase();
    let oracle_pass = source.oracle_outcome == Q4ReasoningOutcome::Pass;
    let prediction_pass = source.prediction_verdict == Q4ReasoningPredictionVerdict::Pass;

    let (class, confidence, reason) = if text.is_empty() {
        (
            Q4ReasoningClass::None,
            1.0,
            "empty observed agent prose".to_string(),
        )
    } else if is_code_only(text, &features) {
        (
            Q4ReasoningClass::CodeOnly,
            0.99,
            "observed prose contains code blocks only".to_string(),
        )
    } else if is_non_english(text, &features) {
        (
            Q4ReasoningClass::Unsupported,
            0.95,
            "non-English agent prose is not classified by the English reasoning head".to_string(),
        )
    } else if features.apology_terms > 0 {
        (
            Q4ReasoningClass::Apologetic,
            0.90,
            "apology marker found in observed agent prose".to_string(),
        )
    } else if !oracle_pass
        && features.success_claim_terms > 0
        && (features.confidence_terms > 0
            || features.pairwise_cosine_reasoning_oracle < 0.25
            || prediction_pass)
    {
        (
            Q4ReasoningClass::Overclaiming,
            0.92,
            "agent claimed success while oracle reality failed".to_string(),
        )
    } else if features.hedge_terms > 0 {
        (
            Q4ReasoningClass::Hedging,
            0.88,
            "hedging marker found in observed agent prose".to_string(),
        )
    } else if oracle_pass && prediction_pass && features.success_claim_terms > 0 {
        (
            Q4ReasoningClass::ConfidentCorrect,
            0.86,
            "confident success claim matches passing oracle reality".to_string(),
        )
    } else if !oracle_pass && features.success_claim_terms > 0 {
        (
            Q4ReasoningClass::ConfidentWrong,
            0.84,
            "confident success claim conflicts with failing oracle reality".to_string(),
        )
    } else if (!oracle_pass && features.failure_ack_terms > 0)
        || (oracle_pass && lower.contains("verified"))
    {
        (
            Q4ReasoningClass::Calibrated,
            0.82,
            "agent prose is calibrated to observed verification evidence".to_string(),
        )
    } else {
        (
            Q4ReasoningClass::Calibrated,
            0.70,
            "agent prose contains no hedge or overclaim marker".to_string(),
        )
    };

    let label = Q4ReasoningLabel {
        corpus_row_id: source.corpus_row_id.clone(),
        chunk_id: source.chunk_id.clone(),
        session_id: source.session_id.clone(),
        class,
        confidence,
        reason,
        oracle_outcome: source.oracle_outcome,
        prediction_verdict: source.prediction_verdict,
        features,
    };
    Ok(label)
}

fn extract_features(source: &Q4ReasoningSource) -> InstrumentResult<Q4ReasoningFeatures> {
    let lower = source.agent_prose.to_ascii_lowercase();
    let lines = source.agent_prose.lines().collect::<Vec<_>>();
    let code_like_lines = lines.iter().filter(|line| is_code_like_line(line)).count() as u32;
    let natural_language_words = lower
        .split(|ch: char| !ch.is_ascii_alphabetic())
        .filter(|word| word.len() >= 2)
        .count() as u32;
    let chars = source.agent_prose.chars().count().max(1) as f64;
    let non_ascii = source
        .agent_prose
        .chars()
        .filter(|ch| ch.is_alphabetic() && !ch.is_ascii())
        .count() as f64;
    Ok(Q4ReasoningFeatures {
        hedge_terms: count_terms(
            &lower,
            &[
                "maybe", "probably", "i think", "not sure", "seems", "should", "might", "could",
                "appears", "likely",
            ],
        ),
        apology_terms: count_terms(&lower, &["sorry", "apologize", "apologies", "my mistake"]),
        confidence_terms: count_terms(
            &lower,
            &[
                "definitely",
                "certainly",
                "guaranteed",
                "complete",
                "fully",
                "all tests pass",
            ],
        ),
        success_claim_terms: count_terms(
            &lower,
            &[
                "works",
                "fixed",
                "resolved",
                "passing",
                "all tests pass",
                "implemented",
                "complete",
            ],
        ),
        failure_ack_terms: count_terms(
            &lower,
            &[
                "fails",
                "failed",
                "failing",
                "regression",
                "not passing",
                "broken",
            ],
        ),
        natural_language_words,
        code_like_lines,
        non_ascii_ratio: non_ascii / chars,
        pairwise_cosine_reasoning_diff: source.pairwise_cosine_reasoning_diff,
        pairwise_cosine_reasoning_oracle: source.pairwise_cosine_reasoning_oracle,
        e17_affect_score: source.e17_affect_score,
        oracle_pass: source.oracle_outcome == Q4ReasoningOutcome::Pass,
    })
}

fn evaluate_holdout(
    centroids: &BTreeMap<Q4ReasoningClass, Vec<f64>>,
    holdout: &[&Q4ReasoningLabel],
    checkpoint_sha256: &str,
) -> InstrumentResult<Vec<Q4ReasoningCalibrationRow>> {
    let mut classes = BTreeSet::new();
    let mut matrix: BTreeMap<(Q4ReasoningClass, Q4ReasoningClass), u64> = BTreeMap::new();
    for label in holdout {
        let predicted =
            predict_from_centroids(centroids, &label.features).unwrap_or(Q4ReasoningClass::Unknown);
        classes.insert(label.class);
        classes.insert(predicted);
        *matrix.entry((label.class, predicted)).or_default() += 1;
    }

    let mut rows = Vec::new();
    for class in classes {
        let true_positive = matrix.get(&(class, class)).copied().unwrap_or(0) as f64;
        let actual = matrix
            .iter()
            .filter(|((actual, _), _)| *actual == class)
            .map(|(_, count)| *count)
            .sum::<u64>() as f64;
        let predicted = matrix
            .iter()
            .filter(|((_, predicted), _)| *predicted == class)
            .map(|(_, count)| *count)
            .sum::<u64>() as f64;
        let precision = if predicted > 0.0 {
            true_positive / predicted
        } else {
            0.0
        };
        let recall = if actual > 0.0 {
            true_positive / actual
        } else {
            0.0
        };
        let f1 = if precision + recall > 0.0 {
            2.0 * precision * recall / (precision + recall)
        } else {
            0.0
        };
        rows.push(Q4ReasoningCalibrationRow {
            head: "reasoning".to_string(),
            class,
            support: actual as u64,
            precision,
            recall,
            f1,
            checkpoint_sha256: checkpoint_sha256.to_string(),
        });
    }
    Ok(rows)
}

fn predict_from_centroids(
    centroids: &BTreeMap<Q4ReasoningClass, Vec<f64>>,
    features: &Q4ReasoningFeatures,
) -> Option<Q4ReasoningClass> {
    let vector = features.vector();
    centroids
        .iter()
        .map(|(class, centroid)| {
            let distance = centroid
                .iter()
                .zip(vector.iter())
                .map(|(left, right)| {
                    let delta = left - right;
                    delta * delta
                })
                .sum::<f64>();
            (*class, distance)
        })
        .min_by(|(_, left), (_, right)| left.total_cmp(right))
        .map(|(class, _)| class)
}

fn is_code_only(text: &str, features: &Q4ReasoningFeatures) -> bool {
    let trimmed = text.trim();
    if trimmed.starts_with("```") && trimmed.ends_with("```") {
        return true;
    }
    features.code_like_lines > 0 && features.natural_language_words <= 3
}

fn is_non_english(text: &str, features: &Q4ReasoningFeatures) -> bool {
    if features.non_ascii_ratio > 0.08 {
        return true;
    }
    let lower = text.to_ascii_lowercase();
    let english_markers = count_terms(
        &lower,
        &[
            "the", "and", "this", "that", "test", "fix", "code", "because", "verified", "works",
            "fails",
        ],
    );
    let spanish_markers = count_terms(
        &lower,
        &[
            " esto ",
            " porque ",
            " funciona",
            " prueba",
            " codigo",
            " código",
        ],
    );
    spanish_markers >= 2 && english_markers == 0
}

fn is_code_like_line(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return false;
    }
    trimmed.starts_with("```")
        || trimmed.starts_with("def ")
        || trimmed.starts_with("class ")
        || trimmed.starts_with("import ")
        || trimmed.contains(" = ")
        || trimmed.ends_with(':')
        || trimmed.ends_with(';')
        || trimmed.contains("return ")
}

fn count_terms(text: &str, terms: &[&str]) -> u32 {
    terms.iter().filter(|term| text.contains(**term)).count() as u32
}

fn validate_source(source: &Q4ReasoningSource) -> InstrumentResult<()> {
    validate_path_component("q4_reasoning.source.corpus_row_id", &source.corpus_row_id)?;
    validate_non_empty_single_line("q4_reasoning.source.chunk_id", &source.chunk_id)?;
    validate_non_empty_single_line("q4_reasoning.source.session_id", &source.session_id)?;
    if source.agent_prose.len() > MAX_AGENT_PROSE_BYTES {
        return invalid(
            "q4_reasoning.source.agent_prose",
            format!(
                "agent prose has {} bytes; max supported bytes is {MAX_AGENT_PROSE_BYTES}",
                source.agent_prose.len()
            ),
            "shard long agent responses before reasoning-label extraction",
        );
    }
    validate_cosine(
        "q4_reasoning.source.pairwise_cosine_reasoning_diff",
        source.pairwise_cosine_reasoning_diff,
    )?;
    validate_cosine(
        "q4_reasoning.source.pairwise_cosine_reasoning_oracle",
        source.pairwise_cosine_reasoning_oracle,
    )?;
    validate_unit_interval(
        "q4_reasoning.source.e17_affect_score",
        source.e17_affect_score,
    )
}

fn validate_labels(labels: &[Q4ReasoningLabel]) -> InstrumentResult<()> {
    if labels.len() > MAX_LABELS {
        return invalid(
            "q4_reasoning.labels",
            format!(
                "Q4 reasoning label count {} exceeds {MAX_LABELS}",
                labels.len()
            ),
            "shard large reasoning label sets before training",
        );
    }
    for label in labels {
        validate_label_shape(label)?;
    }
    Ok(())
}

fn validate_label(label: &Q4ReasoningLabel, source: &Q4ReasoningSource) -> InstrumentResult<()> {
    validate_label_shape(label)?;
    if label.corpus_row_id != source.corpus_row_id
        || label.chunk_id != source.chunk_id
        || label.session_id != source.session_id
    {
        return invalid(
            "q4_reasoning.label.identity",
            "reasoning label identity does not match source identity",
            "derive labels directly from the observed source row",
        );
    }
    Ok(())
}

fn validate_label_shape(label: &Q4ReasoningLabel) -> InstrumentResult<()> {
    validate_path_component("q4_reasoning.label.corpus_row_id", &label.corpus_row_id)?;
    validate_non_empty_single_line("q4_reasoning.label.chunk_id", &label.chunk_id)?;
    validate_non_empty_single_line("q4_reasoning.label.session_id", &label.session_id)?;
    validate_unit_interval("q4_reasoning.label.confidence", label.confidence)?;
    validate_non_empty_single_line("q4_reasoning.label.reason", &label.reason)?;
    validate_features(&label.features)
}

fn validate_features(features: &Q4ReasoningFeatures) -> InstrumentResult<()> {
    validate_unit_interval(
        "q4_reasoning.features.non_ascii_ratio",
        features.non_ascii_ratio,
    )?;
    validate_cosine(
        "q4_reasoning.features.pairwise_cosine_reasoning_diff",
        features.pairwise_cosine_reasoning_diff,
    )?;
    validate_cosine(
        "q4_reasoning.features.pairwise_cosine_reasoning_oracle",
        features.pairwise_cosine_reasoning_oracle,
    )?;
    validate_unit_interval(
        "q4_reasoning.features.e17_affect_score",
        features.e17_affect_score,
    )
}

fn validate_checkpoint(checkpoint: &Q4ReasoningHeadCheckpoint) -> InstrumentResult<()> {
    if checkpoint.schema_version != Q4_REASONING_SCHEMA_VERSION {
        return invalid(
            "q4_reasoning.checkpoint.schema_version",
            format!(
                "expected {}, got {}",
                Q4_REASONING_SCHEMA_VERSION, checkpoint.schema_version
            ),
            "write Q4 reasoning checkpoints through train_q4_reasoning_head",
        );
    }
    if checkpoint.feature_dim != FEATURE_DIM {
        return invalid(
            "q4_reasoning.checkpoint.feature_dim",
            "feature_dim changed without schema migration",
            "bump the Q4 reasoning schema before changing the feature vector",
        );
    }
    for centroid in checkpoint.centroids.values() {
        if centroid.len() != FEATURE_DIM {
            return invalid(
                "q4_reasoning.checkpoint.centroid",
                "centroid dimensionality mismatch",
                "train reasoning centroids from Q4ReasoningFeatures::vector",
            );
        }
        for value in centroid {
            validate_finite("q4_reasoning.checkpoint.centroid_value", *value)?;
        }
    }
    for row in &checkpoint.class_metrics {
        validate_calibration_row(row)?;
    }
    Ok(())
}

pub(crate) fn validate_calibration_row(row: &Q4ReasoningCalibrationRow) -> InstrumentResult<()> {
    if row.head != "reasoning" {
        return invalid(
            "q4_reasoning.calibration.head",
            "calibration row head must be reasoning",
            "persist reasoning metrics through Q4ReasoningLabelStore",
        );
    }
    validate_unit_interval("q4_reasoning.calibration.precision", row.precision)?;
    validate_unit_interval("q4_reasoning.calibration.recall", row.recall)?;
    validate_unit_interval("q4_reasoning.calibration.f1", row.f1)?;
    validate_non_empty_single_line(
        "q4_reasoning.calibration.checkpoint_sha256",
        &row.checkpoint_sha256,
    )
}

pub(crate) fn q4_reasoning_label_key(corpus_row_id: &str, session_id: &str) -> String {
    format!("{corpus_row_id}::{session_id}")
}

pub(crate) fn q4_reasoning_calibration_key(class: Q4ReasoningClass) -> String {
    format!("reasoning::{}", class.as_str())
}

pub(crate) fn validate_path_component(field: &'static str, value: &str) -> InstrumentResult<()> {
    validate_non_empty_single_line(field, value)?;
    if value.contains('/') || value.contains('\\') || value == "." || value == ".." {
        return invalid(
            field,
            format!("{field} must be a single path component"),
            "use stable row identifiers, not filesystem paths, for raw-output directories",
        );
    }
    Ok(())
}

pub(crate) fn validate_non_empty_single_line(
    field: &'static str,
    value: &str,
) -> InstrumentResult<()> {
    if value.trim().is_empty() {
        return invalid(
            field,
            "value must be non-empty",
            "persist the source-of-truth identifier before extracting reasoning labels",
        );
    }
    if value.contains('\n') || value.contains('\r') {
        return invalid(
            field,
            "value must be single-line",
            "store multiline payloads in dedicated prose fields, not identifiers",
        );
    }
    Ok(())
}

fn validate_cosine(field: &'static str, value: f64) -> InstrumentResult<()> {
    validate_finite(field, value)?;
    if !(-1.0..=1.0).contains(&value) {
        return invalid(
            field,
            format!("cosine {value} outside [-1, 1]"),
            "derive pairwise cosine features from normalized E_Reasoning/E_Diff/E_Oracle slots",
        );
    }
    Ok(())
}

fn validate_unit_interval(field: &'static str, value: f64) -> InstrumentResult<()> {
    validate_finite(field, value)?;
    if !(0.0..=1.0).contains(&value) {
        return invalid(
            field,
            format!("value {value} outside [0, 1]"),
            "normalize confidence and E17 affect features before persistence",
        );
    }
    Ok(())
}

fn validate_finite(field: &'static str, value: f64) -> InstrumentResult<()> {
    if !value.is_finite() {
        return invalid(
            field,
            "value must be finite",
            "quarantine non-finite reasoning features before training",
        );
    }
    Ok(())
}

pub(crate) fn invalid<T>(
    field: &'static str,
    message: impl Into<String>,
    remediation: &'static str,
) -> InstrumentResult<T> {
    Err(InstrumentError::invalid(field, message, remediation))
}

pub(crate) fn cf_options() -> rocksdb::Options {
    let mut opts = rocksdb::Options::default();
    opts.set_paranoid_checks(true);
    opts
}

#[path = "q4_reasoning_labels_store.rs"]
mod q4_reasoning_labels_store;
pub use q4_reasoning_labels_store::Q4ReasoningLabelStore;

#[cfg(test)]
#[path = "q4_reasoning_labels_tests.rs"]
mod q4_reasoning_labels_tests;
