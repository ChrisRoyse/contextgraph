//! TASK-RWD-211 / TASK-PY-G-067 novel-pattern detection and ontology growth.
//!
//! This layer deliberately sits on top of the TASK-FP-005 Unknown-fingerprint
//! queue. It does not create a parallel catalog: novel rows become active
//! learning entries, coherent clusters become candidate constellation/fingerprint
//! extensions, and the held-out gate decides whether the system proposal is
//! accepted or refuted.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::error::{EvalError, EvalErrorCode};
use crate::failure_fingerprint::{
    FailureShapeFingerprint, FingerprintCalibrationState, FingerprintConfidence, FingerprintKind,
    FingerprintPairMetric, FAILURE_FINGERPRINT_SCHEMA_VERSION,
};
use crate::types::{ChunkId, EmbedderId, PredictionId, RealityPrediction, TaskId};

pub const DEFAULT_NOVEL_PATTERN_TAU_NOVEL: f32 = 2.0;
pub const DEFAULT_NOVEL_PATTERN_MIN_CLUSTER_SIZE: usize = 3;
pub const DEFAULT_NOVEL_PATTERN_TAU_INTRA: f32 = 0.98;
pub const DEFAULT_NOVEL_PATTERN_TAU_FAR: f32 = 0.15;
pub const DEFAULT_NOVEL_PATTERN_HELDOUT_DELTA: f32 = 0.01;
pub const NOVEL_PATTERN_SOURCE_CORPUS: &str = "novel-pattern-active-learning-v1";

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NovelPatternDetectorConfig {
    pub tau_novel: f32,
    pub min_cluster_size: usize,
    pub tau_intra: f32,
    pub tau_far: f32,
    pub heldout_improvement_delta: f32,
}

impl Default for NovelPatternDetectorConfig {
    fn default() -> Self {
        Self {
            tau_novel: DEFAULT_NOVEL_PATTERN_TAU_NOVEL,
            min_cluster_size: DEFAULT_NOVEL_PATTERN_MIN_CLUSTER_SIZE,
            tau_intra: DEFAULT_NOVEL_PATTERN_TAU_INTRA,
            tau_far: DEFAULT_NOVEL_PATTERN_TAU_FAR,
            heldout_improvement_delta: DEFAULT_NOVEL_PATTERN_HELDOUT_DELTA,
        }
    }
}

impl NovelPatternDetectorConfig {
    pub fn validate(&self) -> Result<(), EvalError> {
        validate_positive("tau_novel", self.tau_novel)?;
        if self.min_cluster_size == 0 {
            return Err(EvalError::new(
                EvalErrorCode::InvalidConfig,
                "novel-pattern min_cluster_size must be greater than zero",
            ));
        }
        validate_probability("tau_intra", self.tau_intra)?;
        validate_distance("tau_far", self.tau_far)?;
        validate_probability("heldout_improvement_delta", self.heldout_improvement_delta)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConstellationCentroid {
    pub cell_id: String,
    pub centroid_by_embedder: BTreeMap<EmbedderId, Vec<f32>>,
    pub variance_by_embedder: BTreeMap<EmbedderId, f32>,
}

impl ConstellationCentroid {
    pub fn validate(&self) -> Result<(), EvalError> {
        validate_text("constellation_centroid.cell_id", &self.cell_id)?;
        validate_observation_map(
            "constellation_centroid.centroid",
            &self.centroid_by_embedder,
        )?;
        if self.variance_by_embedder.keys().collect::<Vec<_>>()
            != self.centroid_by_embedder.keys().collect::<Vec<_>>()
        {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                "constellation centroid variance keys must match centroid embedders",
            ));
        }
        for (embedder, value) in &self.variance_by_embedder {
            if !value.is_finite() || *value < 0.0 {
                return Err(EvalError::new(
                    EvalErrorCode::InvalidInput,
                    format!(
                        "constellation variance for {} must be finite and non-negative",
                        embedder.0
                    ),
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NovelPatternCandidate {
    pub candidate_id: [u8; 16],
    pub prediction_id: PredictionId,
    pub task_id: TaskId,
    pub session_id: [u8; 16],
    pub observed_at_unix_ms: i64,
    pub novelty_score: f32,
    pub nearest_existing_cell_id: String,
    pub nearest_existing_distance: f32,
    pub active_learning_priority: u8,
    pub observation_by_embedder: BTreeMap<EmbedderId, Vec<f32>>,
}

impl NovelPatternCandidate {
    pub fn from_prediction(
        prediction: &RealityPrediction,
        observation_by_embedder: BTreeMap<EmbedderId, Vec<f32>>,
        existing_cells: &[ConstellationCentroid],
        observed_at_unix_ms: i64,
        config: NovelPatternDetectorConfig,
    ) -> Result<Option<Self>, EvalError> {
        prediction.validate().map_err(EvalError::from)?;
        config.validate()?;
        validate_observation_map("novel_pattern.observation", &observation_by_embedder)?;
        if observed_at_unix_ms <= 0 {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                "novel-pattern observed_at_unix_ms must be positive",
            ));
        }
        let novelty = score_novelty(&observation_by_embedder, existing_cells)?;
        if novelty.novelty_score <= config.tau_novel {
            return Ok(None);
        }
        let candidate = Self {
            candidate_id: novel_candidate_id(
                &prediction.task_id,
                PredictionId(prediction.prediction_id),
                observed_at_unix_ms,
                &observation_by_embedder,
            ),
            prediction_id: PredictionId(prediction.prediction_id),
            task_id: prediction.task_id.clone(),
            session_id: prediction.session_id,
            observed_at_unix_ms,
            novelty_score: novelty.novelty_score,
            nearest_existing_cell_id: novelty.nearest_existing_cell_id,
            nearest_existing_distance: novelty.nearest_existing_distance,
            active_learning_priority: if novelty.novelty_score >= config.tau_novel * 2.0 {
                1
            } else {
                2
            },
            observation_by_embedder,
        };
        candidate.validate()?;
        Ok(Some(candidate))
    }

    pub fn validate(&self) -> Result<(), EvalError> {
        if self.candidate_id.iter().all(|byte| *byte == 0) {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                "novel-pattern candidate_id must be non-zero",
            ));
        }
        self.task_id
            .validate("novel_pattern.task_id")
            .map_err(EvalError::from)?;
        if self.prediction_id.0.iter().all(|byte| *byte == 0) {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                "novel-pattern prediction_id must be non-zero",
            ));
        }
        if self.session_id.iter().all(|byte| *byte == 0) {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                "novel-pattern session_id must be non-zero",
            ));
        }
        if self.observed_at_unix_ms <= 0 {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                "novel-pattern observed_at_unix_ms must be positive",
            ));
        }
        validate_positive("novel_pattern.novelty_score", self.novelty_score)?;
        validate_distance(
            "novel_pattern.nearest_existing_distance",
            self.nearest_existing_distance,
        )?;
        validate_text(
            "novel_pattern.nearest_existing_cell_id",
            &self.nearest_existing_cell_id,
        )?;
        if !(1..=5).contains(&self.active_learning_priority) {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                "novel-pattern active_learning_priority must be in 1..=5",
            ));
        }
        validate_observation_map("novel_pattern.observation", &self.observation_by_embedder)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NovelPatternClusterState {
    Candidate,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NovelPatternClusterAdmission {
    pub cluster_id: [u8; 16],
    pub state: NovelPatternClusterState,
    pub suggested_name: String,
    pub member_candidate_ids: Vec<[u8; 16]>,
    pub member_task_ids: Vec<TaskId>,
    pub centroid_by_embedder: BTreeMap<EmbedderId, Vec<f32>>,
    pub variance_by_embedder: BTreeMap<EmbedderId, f32>,
    pub mean_novelty_score: f32,
    pub min_distance_to_existing_centroid: f32,
    pub intra_cluster_min_cosine: f32,
    pub active_learning_priority: u8,
}

impl NovelPatternClusterAdmission {
    pub fn validate(&self) -> Result<(), EvalError> {
        if self.cluster_id.iter().all(|byte| *byte == 0) {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                "novel-pattern cluster_id must be non-zero",
            ));
        }
        validate_text("novel_pattern_cluster.suggested_name", &self.suggested_name)?;
        if self.member_candidate_ids.is_empty() || self.member_task_ids.is_empty() {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                "novel-pattern cluster must have members",
            ));
        }
        if self.member_candidate_ids.len() != self.member_task_ids.len() {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                "novel-pattern cluster member id/task count mismatch",
            ));
        }
        validate_observation_map("novel_pattern_cluster.centroid", &self.centroid_by_embedder)?;
        if self.variance_by_embedder.keys().collect::<Vec<_>>()
            != self.centroid_by_embedder.keys().collect::<Vec<_>>()
        {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                "novel-pattern cluster variance keys must match centroid embedders",
            ));
        }
        validate_positive(
            "novel_pattern_cluster.mean_novelty_score",
            self.mean_novelty_score,
        )?;
        validate_distance(
            "novel_pattern_cluster.min_distance_to_existing_centroid",
            self.min_distance_to_existing_centroid,
        )?;
        validate_probability(
            "novel_pattern_cluster.intra_cluster_min_cosine",
            self.intra_cluster_min_cosine,
        )?;
        if !(1..=5).contains(&self.active_learning_priority) {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                "novel-pattern cluster active_learning_priority must be in 1..=5",
            ));
        }
        Ok(())
    }

    pub fn to_unknown_fingerprint(
        &self,
        frozen_at_unix_ms: i64,
    ) -> Result<FailureShapeFingerprint, EvalError> {
        self.validate()?;
        if frozen_at_unix_ms <= 0 {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                "novel-pattern fingerprint frozen_at_unix_ms must be positive",
            ));
        }
        let kind = FingerprintKind::Unknown {
            observed_at_unix_ms: frozen_at_unix_ms,
            observed_by_session: self.cluster_id,
            ood_score: self.mean_novelty_score.clamp(0.0, 1.0),
            embedder_disagreement_score: (1.0 - self.intra_cluster_min_cosine).clamp(0.0, 1.0),
            active_learning_priority: self.active_learning_priority,
        };
        let fingerprint_id =
            FailureShapeFingerprint::canonical_id(&kind, NOVEL_PATTERN_SOURCE_CORPUS)
                .map_err(EvalError::from)?;
        let tau_by_embedder = self
            .centroid_by_embedder
            .keys()
            .map(|embedder| (embedder.clone(), 1.0))
            .collect::<BTreeMap<_, _>>();
        let reference_chunks = self
            .member_candidate_ids
            .iter()
            .map(|id| ChunkId(format!("novel-pattern:{}", hex::encode(id))))
            .collect::<Vec<_>>();
        let mut pairwise_cosine = Vec::new();
        let embedders = self
            .centroid_by_embedder
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        for left in 0..embedders.len() {
            for right in (left + 1)..embedders.len() {
                let left_vec = self
                    .centroid_by_embedder
                    .get(&embedders[left])
                    .expect("centroid key exists");
                let right_vec = self
                    .centroid_by_embedder
                    .get(&embedders[right])
                    .expect("centroid key exists");
                pairwise_cosine.push(FingerprintPairMetric {
                    left_embedder: embedders[left].clone(),
                    right_embedder: embedders[right].clone(),
                    value: vector_cosine(left_vec, right_vec, "novel-pattern fingerprint")?,
                });
            }
        }
        let fingerprint = FailureShapeFingerprint {
            schema_version: FAILURE_FINGERPRINT_SCHEMA_VERSION,
            fingerprint_id,
            kind,
            name: self.suggested_name.clone(),
            source_corpus: NOVEL_PATTERN_SOURCE_CORPUS.to_string(),
            source_manifest_sha256: None,
            centroid_by_embedder: self.centroid_by_embedder.clone(),
            variance_by_embedder: self.variance_by_embedder.clone(),
            tau_by_embedder,
            pairwise_cosine,
            pairwise_mutual_information: Vec::new(),
            reference_chunks,
            n_references: self.member_candidate_ids.len(),
            oracle_outcome: None,
            is_canonical: true,
            frozen_at_unix_ms,
            confidence: FingerprintConfidence {
                calibration_state: FingerprintCalibrationState::NeedsRecalibration,
                ..FingerprintConfidence::default()
            },
        };
        fingerprint.validate().map_err(EvalError::from)?;
        Ok(fingerprint)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HeldoutImprovementEvidence {
    pub baseline_per_cell_correlation: BTreeMap<String, f32>,
    pub candidate_per_cell_correlation: BTreeMap<String, f32>,
}

impl HeldoutImprovementEvidence {
    pub fn validate(&self) -> Result<(), EvalError> {
        if self.baseline_per_cell_correlation.is_empty() {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                "held-out evidence must include at least one cell",
            ));
        }
        if self
            .baseline_per_cell_correlation
            .keys()
            .collect::<Vec<_>>()
            != self
                .candidate_per_cell_correlation
                .keys()
                .collect::<Vec<_>>()
        {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                "held-out evidence baseline/candidate cells must match",
            ));
        }
        for (cell, baseline) in &self.baseline_per_cell_correlation {
            validate_correlation(cell, *baseline)?;
            validate_correlation(
                cell,
                *self
                    .candidate_per_cell_correlation
                    .get(cell)
                    .expect("cell key exists"),
            )?;
        }
        Ok(())
    }

    pub fn deltas(&self) -> Result<BTreeMap<String, f32>, EvalError> {
        self.validate()?;
        Ok(self
            .baseline_per_cell_correlation
            .iter()
            .map(|(cell, baseline)| {
                (
                    cell.clone(),
                    self.candidate_per_cell_correlation[cell] - baseline,
                )
            })
            .collect())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OntologyGrowthAuditOutcome {
    Candidate,
    Accepted,
    RejectedByFalsification,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OntologyGrowthInitiatedBy {
    SystemCandidate,
    SystemProposalAccepted,
    SystemProposalRefuted,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OntologyGrowthAuditEntry {
    pub audit_id: [u8; 16],
    pub cluster_id: [u8; 16],
    pub outcome: OntologyGrowthAuditOutcome,
    pub initiated_by: OntologyGrowthInitiatedBy,
    pub created_at_unix_ms: i64,
    pub suggested_name: String,
    pub member_count: usize,
    pub min_delta_required: f32,
    pub min_observed_delta: f32,
    pub per_cell_delta: BTreeMap<String, f32>,
    pub reason: String,
    pub centroid_by_embedder: BTreeMap<EmbedderId, Vec<f32>>,
}

impl OntologyGrowthAuditEntry {
    pub fn validate(&self) -> Result<(), EvalError> {
        if self.audit_id.iter().all(|byte| *byte == 0) {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                "ontology-growth audit_id must be non-zero",
            ));
        }
        if self.cluster_id.iter().all(|byte| *byte == 0) {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                "ontology-growth cluster_id must be non-zero",
            ));
        }
        if self.created_at_unix_ms <= 0 {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                "ontology-growth created_at_unix_ms must be positive",
            ));
        }
        validate_text("ontology_growth.suggested_name", &self.suggested_name)?;
        validate_text("ontology_growth.reason", &self.reason)?;
        if self.member_count == 0 {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                "ontology-growth member_count must be greater than zero",
            ));
        }
        validate_probability(
            "ontology_growth.min_delta_required",
            self.min_delta_required,
        )?;
        validate_correlation_delta(
            "ontology_growth.min_observed_delta",
            self.min_observed_delta,
        )?;
        for (cell, delta) in &self.per_cell_delta {
            validate_text("ontology_growth.cell", cell)?;
            validate_correlation_delta("ontology_growth.delta", *delta)?;
        }
        validate_observation_map("ontology_growth.centroid", &self.centroid_by_embedder)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NovelInstrumentProposalInput {
    pub cluster_id: [u8; 16],
    pub suggested_instrument_name: String,
    pub member_count: usize,
    pub audit_outcome: OntologyGrowthAuditOutcome,
    pub centroid_by_embedder: BTreeMap<EmbedderId, Vec<f32>>,
}

pub fn admit_novel_pattern_clusters(
    candidates: &[NovelPatternCandidate],
    existing_cells: &[ConstellationCentroid],
    config: NovelPatternDetectorConfig,
) -> Result<Vec<NovelPatternClusterAdmission>, EvalError> {
    config.validate()?;
    validate_existing_cells(existing_cells)?;
    let mut assigned = BTreeSet::new();
    let mut admissions = Vec::new();
    for seed in candidates {
        seed.validate()?;
        if assigned.contains(&seed.candidate_id) {
            continue;
        }
        let mut cluster = vec![seed];
        for candidate in candidates {
            candidate.validate()?;
            if candidate.candidate_id == seed.candidate_id
                || assigned.contains(&candidate.candidate_id)
            {
                continue;
            }
            let similarity = mean_observation_cosine(
                &seed.observation_by_embedder,
                &candidate.observation_by_embedder,
            )?;
            if similarity >= config.tau_intra {
                cluster.push(candidate);
            }
        }
        if cluster.len() < config.min_cluster_size {
            continue;
        }
        let admission = build_cluster_admission(&cluster, existing_cells)?;
        if admission.intra_cluster_min_cosine < config.tau_intra
            || admission.min_distance_to_existing_centroid < config.tau_far
        {
            continue;
        }
        for candidate in &cluster {
            assigned.insert(candidate.candidate_id);
        }
        admissions.push(admission);
    }
    Ok(admissions)
}

pub fn candidate_audit_entry(
    admission: &NovelPatternClusterAdmission,
    created_at_unix_ms: i64,
    config: NovelPatternDetectorConfig,
) -> Result<OntologyGrowthAuditEntry, EvalError> {
    config.validate()?;
    build_audit(
        admission,
        OntologyGrowthAuditOutcome::Candidate,
        OntologyGrowthInitiatedBy::SystemCandidate,
        created_at_unix_ms,
        config.heldout_improvement_delta,
        BTreeMap::new(),
        0.0,
        "candidate_cluster_pending_heldout_falsification",
    )
}

pub fn evaluate_novel_pattern_promotion(
    admission: &NovelPatternClusterAdmission,
    evidence: &HeldoutImprovementEvidence,
    created_at_unix_ms: i64,
    config: NovelPatternDetectorConfig,
) -> Result<OntologyGrowthAuditEntry, EvalError> {
    config.validate()?;
    admission.validate()?;
    let deltas = evidence.deltas()?;
    let min_observed_delta = deltas
        .values()
        .copied()
        .fold(f32::INFINITY, |left, right| left.min(right));
    let accepted = deltas
        .values()
        .all(|delta| *delta + f32::EPSILON >= config.heldout_improvement_delta);
    let (outcome, initiated_by, reason) = if accepted {
        (
            OntologyGrowthAuditOutcome::Accepted,
            OntologyGrowthInitiatedBy::SystemProposalAccepted,
            "heldout_improvement_gate_passed",
        )
    } else {
        (
            OntologyGrowthAuditOutcome::RejectedByFalsification,
            OntologyGrowthInitiatedBy::SystemProposalRefuted,
            "rejected_by_falsification",
        )
    };
    build_audit(
        admission,
        outcome,
        initiated_by,
        created_at_unix_ms,
        config.heldout_improvement_delta,
        deltas,
        min_observed_delta,
        reason,
    )
}

pub fn proposal_inputs_from_ontology_audit(
    entries: &[OntologyGrowthAuditEntry],
) -> Result<Vec<NovelInstrumentProposalInput>, EvalError> {
    let mut out = Vec::new();
    for entry in entries {
        entry.validate()?;
        if entry.outcome == OntologyGrowthAuditOutcome::RejectedByFalsification {
            continue;
        }
        out.push(NovelInstrumentProposalInput {
            cluster_id: entry.cluster_id,
            suggested_instrument_name: format!(
                "mejepa_proposed_{}",
                hex::encode(&entry.cluster_id[..6])
            ),
            member_count: entry.member_count,
            audit_outcome: entry.outcome,
            centroid_by_embedder: entry.centroid_by_embedder.clone(),
        });
    }
    Ok(out)
}

pub fn ontology_growth_audit_key(entry: &OntologyGrowthAuditEntry) -> Vec<u8> {
    format!(
        "{:020}::{}",
        entry.created_at_unix_ms,
        hex::encode(entry.audit_id)
    )
    .into_bytes()
}

struct NoveltyScore {
    novelty_score: f32,
    nearest_existing_cell_id: String,
    nearest_existing_distance: f32,
}

fn score_novelty(
    observation: &BTreeMap<EmbedderId, Vec<f32>>,
    existing_cells: &[ConstellationCentroid],
) -> Result<NoveltyScore, EvalError> {
    validate_observation_map("novel_pattern.observation", observation)?;
    validate_existing_cells(existing_cells)?;
    let mut max_z = f32::NEG_INFINITY;
    let mut nearest_id = String::new();
    let mut nearest_distance = f32::INFINITY;
    for cell in existing_cells {
        let cosine = mean_observation_cosine(observation, &cell.centroid_by_embedder)?;
        let distance = (1.0 - cosine).clamp(0.0, 2.0);
        let variance = mean_variance(&cell.variance_by_embedder)?;
        let z = distance / variance.sqrt().max(1.0e-6);
        max_z = max_z.max(z);
        if distance < nearest_distance {
            nearest_distance = distance;
            nearest_id = cell.cell_id.clone();
        }
    }
    Ok(NoveltyScore {
        novelty_score: max_z,
        nearest_existing_cell_id: nearest_id,
        nearest_existing_distance: nearest_distance,
    })
}

fn build_cluster_admission(
    cluster: &[&NovelPatternCandidate],
    existing_cells: &[ConstellationCentroid],
) -> Result<NovelPatternClusterAdmission, EvalError> {
    let first = cluster
        .first()
        .ok_or_else(|| EvalError::new(EvalErrorCode::InvalidInput, "empty novel cluster"))?;
    let centroid_by_embedder = centroid(cluster)?;
    let variance_by_embedder = cluster_variance(cluster, &centroid_by_embedder)?;
    let min_distance_to_existing_centroid = existing_cells
        .iter()
        .map(|cell| {
            mean_observation_cosine(&centroid_by_embedder, &cell.centroid_by_embedder)
                .map(|cosine| (1.0 - cosine).clamp(0.0, 2.0))
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .fold(f32::INFINITY, |left, right| left.min(right));
    let intra_cluster_min_cosine = min_pairwise_cosine(cluster)?;
    let member_candidate_ids = cluster
        .iter()
        .map(|candidate| candidate.candidate_id)
        .collect::<Vec<_>>();
    let cluster_id = novel_cluster_id(&member_candidate_ids);
    let admission = NovelPatternClusterAdmission {
        cluster_id,
        state: NovelPatternClusterState::Candidate,
        suggested_name: format!("novel-pattern-{}", hex::encode(&cluster_id[..8])),
        member_candidate_ids,
        member_task_ids: cluster
            .iter()
            .map(|candidate| candidate.task_id.clone())
            .collect(),
        centroid_by_embedder,
        variance_by_embedder,
        mean_novelty_score: cluster
            .iter()
            .map(|candidate| candidate.novelty_score)
            .sum::<f32>()
            / cluster.len() as f32,
        min_distance_to_existing_centroid,
        intra_cluster_min_cosine,
        active_learning_priority: cluster
            .iter()
            .map(|candidate| candidate.active_learning_priority)
            .min()
            .unwrap_or(first.active_learning_priority),
    };
    admission.validate()?;
    Ok(admission)
}

fn centroid(
    cluster: &[&NovelPatternCandidate],
) -> Result<BTreeMap<EmbedderId, Vec<f32>>, EvalError> {
    let first = cluster
        .first()
        .ok_or_else(|| EvalError::new(EvalErrorCode::InvalidInput, "empty novel cluster"))?;
    let mut sums = first
        .observation_by_embedder
        .iter()
        .map(|(embedder, vector)| (embedder.clone(), vec![0.0_f64; vector.len()]))
        .collect::<BTreeMap<_, _>>();
    for candidate in cluster {
        candidate.validate()?;
        if candidate.observation_by_embedder.keys().collect::<Vec<_>>()
            != first.observation_by_embedder.keys().collect::<Vec<_>>()
        {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                "novel-pattern cluster members must share embedder keys",
            ));
        }
        for (embedder, vector) in &candidate.observation_by_embedder {
            let sum = sums.get_mut(embedder).expect("embedder key exists");
            if sum.len() != vector.len() {
                return Err(EvalError::new(
                    EvalErrorCode::InvalidInput,
                    format!("novel-pattern dim mismatch for {}", embedder.0),
                ));
            }
            for (idx, value) in vector.iter().enumerate() {
                sum[idx] += f64::from(*value);
            }
        }
    }
    sums.into_iter()
        .map(|(embedder, vector)| {
            let mean = vector
                .into_iter()
                .map(|value| (value / cluster.len() as f64) as f32)
                .collect::<Vec<_>>();
            Ok((embedder, normalize_vector(&mean, "novel-pattern centroid")?))
        })
        .collect()
}

fn cluster_variance(
    cluster: &[&NovelPatternCandidate],
    centroid_by_embedder: &BTreeMap<EmbedderId, Vec<f32>>,
) -> Result<BTreeMap<EmbedderId, f32>, EvalError> {
    let mut out = BTreeMap::new();
    for (embedder, centroid) in centroid_by_embedder {
        let mut total = 0.0_f32;
        for candidate in cluster {
            let vector = candidate
                .observation_by_embedder
                .get(embedder)
                .ok_or_else(|| {
                    EvalError::new(
                        EvalErrorCode::InvalidInput,
                        format!("novel-pattern cluster missing embedder {}", embedder.0),
                    )
                })?;
            let cosine = vector_cosine(vector, centroid, "novel-pattern variance")?;
            total += (1.0 - cosine).powi(2);
        }
        out.insert(embedder.clone(), total / cluster.len() as f32);
    }
    Ok(out)
}

fn min_pairwise_cosine(cluster: &[&NovelPatternCandidate]) -> Result<f32, EvalError> {
    if cluster.len() < 2 {
        return Ok(1.0);
    }
    let mut min_cosine = f32::INFINITY;
    for left in 0..cluster.len() {
        for right in (left + 1)..cluster.len() {
            min_cosine = min_cosine.min(mean_observation_cosine(
                &cluster[left].observation_by_embedder,
                &cluster[right].observation_by_embedder,
            )?);
        }
    }
    Ok(min_cosine)
}

fn build_audit(
    admission: &NovelPatternClusterAdmission,
    outcome: OntologyGrowthAuditOutcome,
    initiated_by: OntologyGrowthInitiatedBy,
    created_at_unix_ms: i64,
    min_delta_required: f32,
    per_cell_delta: BTreeMap<String, f32>,
    min_observed_delta: f32,
    reason: &str,
) -> Result<OntologyGrowthAuditEntry, EvalError> {
    admission.validate()?;
    if created_at_unix_ms <= 0 {
        return Err(EvalError::new(
            EvalErrorCode::InvalidInput,
            "ontology-growth created_at_unix_ms must be positive",
        ));
    }
    let entry = OntologyGrowthAuditEntry {
        audit_id: ontology_audit_id(&admission.cluster_id, outcome, created_at_unix_ms),
        cluster_id: admission.cluster_id,
        outcome,
        initiated_by,
        created_at_unix_ms,
        suggested_name: admission.suggested_name.clone(),
        member_count: admission.member_candidate_ids.len(),
        min_delta_required,
        min_observed_delta,
        per_cell_delta,
        reason: reason.to_string(),
        centroid_by_embedder: admission.centroid_by_embedder.clone(),
    };
    entry.validate()?;
    Ok(entry)
}

fn validate_existing_cells(cells: &[ConstellationCentroid]) -> Result<(), EvalError> {
    if cells.is_empty() {
        return Err(EvalError::new(
            EvalErrorCode::InvalidInput,
            "novel-pattern detection requires at least one existing constellation cell",
        ));
    }
    for cell in cells {
        cell.validate()?;
    }
    Ok(())
}

fn validate_observation_map(
    field: &str,
    map: &BTreeMap<EmbedderId, Vec<f32>>,
) -> Result<(), EvalError> {
    if map.is_empty() {
        return Err(EvalError::new(
            EvalErrorCode::InvalidInput,
            format!("{field} must be non-empty"),
        ));
    }
    for (embedder, vector) in map {
        embedder
            .validate(&format!("{field}.embedder"))
            .map_err(EvalError::from)?;
        if vector.is_empty() {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                format!("{field} vector for {} must be non-empty", embedder.0),
            ));
        }
        for value in vector {
            if !value.is_finite() {
                return Err(EvalError::new(
                    EvalErrorCode::InvalidInput,
                    format!("{field} vector for {} contains {value}", embedder.0),
                ));
            }
        }
        normalize_vector(vector, field)?;
    }
    Ok(())
}

fn mean_observation_cosine(
    left: &BTreeMap<EmbedderId, Vec<f32>>,
    right: &BTreeMap<EmbedderId, Vec<f32>>,
) -> Result<f32, EvalError> {
    if left.keys().collect::<Vec<_>>() != right.keys().collect::<Vec<_>>() {
        return Err(EvalError::new(
            EvalErrorCode::InvalidInput,
            "novel-pattern observations must share embedder keys",
        ));
    }
    let mut total = 0.0;
    for (embedder, left_vector) in left {
        let right_vector = right.get(embedder).expect("right key exists");
        total += vector_cosine(left_vector, right_vector, &embedder.0)?;
    }
    Ok(total / left.len() as f32)
}

fn vector_cosine(left: &[f32], right: &[f32], context: &str) -> Result<f32, EvalError> {
    if left.len() != right.len() {
        return Err(EvalError::new(
            EvalErrorCode::InvalidInput,
            format!("novel-pattern vector dim mismatch for {context}"),
        ));
    }
    let left = normalize_vector(left, context)?;
    let right = normalize_vector(right, context)?;
    Ok(left
        .iter()
        .zip(right)
        .map(|(left, right)| left * right)
        .sum::<f32>()
        .clamp(-1.0, 1.0))
}

fn normalize_vector(vector: &[f32], context: &str) -> Result<Vec<f32>, EvalError> {
    let mut norm = 0.0_f64;
    for value in vector {
        if !value.is_finite() {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                format!("novel-pattern vector for {context} contains {value}"),
            ));
        }
        norm += f64::from(*value) * f64::from(*value);
    }
    if norm <= f64::EPSILON {
        return Err(EvalError::new(
            EvalErrorCode::InvalidInput,
            format!("novel-pattern vector for {context} has zero norm"),
        ));
    }
    let norm = norm.sqrt();
    Ok(vector
        .iter()
        .map(|value| (f64::from(*value) / norm) as f32)
        .collect())
}

fn mean_variance(values: &BTreeMap<EmbedderId, f32>) -> Result<f32, EvalError> {
    if values.is_empty() {
        return Err(EvalError::new(
            EvalErrorCode::InvalidInput,
            "novel-pattern cell variance map must be non-empty",
        ));
    }
    let mut total = 0.0;
    for value in values.values() {
        if !value.is_finite() || *value < 0.0 {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                "novel-pattern cell variance must be finite and non-negative",
            ));
        }
        total += *value;
    }
    Ok(total / values.len() as f32)
}

fn novel_candidate_id(
    task_id: &TaskId,
    prediction_id: PredictionId,
    observed_at_unix_ms: i64,
    observation_by_embedder: &BTreeMap<EmbedderId, Vec<f32>>,
) -> [u8; 16] {
    let mut hasher = Sha256::new();
    hasher.update(b"MEJEPA_NOVEL_PATTERN_CANDIDATE_V1");
    hasher.update(task_id.0.as_bytes());
    hasher.update(prediction_id.0);
    hasher.update(observed_at_unix_ms.to_be_bytes());
    for (embedder, vector) in observation_by_embedder {
        hasher.update(embedder.0.as_bytes());
        for value in vector {
            hasher.update(value.to_bits().to_be_bytes());
        }
    }
    digest16(hasher)
}

fn novel_cluster_id(member_candidate_ids: &[[u8; 16]]) -> [u8; 16] {
    let mut ids = member_candidate_ids.to_vec();
    ids.sort();
    let mut hasher = Sha256::new();
    hasher.update(b"MEJEPA_NOVEL_PATTERN_CLUSTER_V1");
    for id in ids {
        hasher.update(id);
    }
    digest16(hasher)
}

fn ontology_audit_id(
    cluster_id: &[u8; 16],
    outcome: OntologyGrowthAuditOutcome,
    created_at_unix_ms: i64,
) -> [u8; 16] {
    let mut hasher = Sha256::new();
    hasher.update(b"MEJEPA_ONTOLOGY_GROWTH_AUDIT_V1");
    hasher.update(cluster_id);
    hasher.update(format!("{outcome:?}").as_bytes());
    hasher.update(created_at_unix_ms.to_be_bytes());
    digest16(hasher)
}

fn digest16(hasher: Sha256) -> [u8; 16] {
    let digest = hasher.finalize();
    let mut out = [0u8; 16];
    out.copy_from_slice(&digest[..16]);
    out
}

fn validate_text(field: &str, value: &str) -> Result<(), EvalError> {
    if value.trim().is_empty() || value.len() > 512 || value.bytes().any(|b| b < 0x20 || b == 0x7f)
    {
        return Err(EvalError::new(
            EvalErrorCode::InvalidInput,
            format!("{field} must be non-empty printable text <=512 bytes"),
        ));
    }
    Ok(())
}

fn validate_probability(field: &str, value: f32) -> Result<(), EvalError> {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return Err(EvalError::new(
            EvalErrorCode::InvalidInput,
            format!("{field} must be finite and in [0,1]; got {value}"),
        ));
    }
    Ok(())
}

fn validate_distance(field: &str, value: f32) -> Result<(), EvalError> {
    if !value.is_finite() || !(0.0..=2.0).contains(&value) {
        return Err(EvalError::new(
            EvalErrorCode::InvalidInput,
            format!("{field} must be finite and in [0,2]; got {value}"),
        ));
    }
    Ok(())
}

fn validate_positive(field: &str, value: f32) -> Result<(), EvalError> {
    if !value.is_finite() || value <= 0.0 {
        return Err(EvalError::new(
            EvalErrorCode::InvalidInput,
            format!("{field} must be finite and >0; got {value}"),
        ));
    }
    Ok(())
}

fn validate_correlation(field: &str, value: f32) -> Result<(), EvalError> {
    if !value.is_finite() || !(-1.0..=1.0).contains(&value) {
        return Err(EvalError::new(
            EvalErrorCode::InvalidInput,
            format!("{field} must be finite and in [-1,1]; got {value}"),
        ));
    }
    Ok(())
}

fn validate_correlation_delta(field: &str, value: f32) -> Result<(), EvalError> {
    if !value.is_finite() || !(-2.0..=2.0).contains(&value) {
        return Err(EvalError::new(
            EvalErrorCode::InvalidInput,
            format!("{field} must be finite and in [-2,2]; got {value}"),
        ));
    }
    Ok(())
}
