//! Failure-shape fingerprint catalog types and RocksDB source-of-truth helpers.
//!
//! These records are the durable TASK-FP-001/TASK-FP-002 contract: seed
//! materializers write canonical `KnownGood` and `KnownBad` fingerprints here,
//! live inference reads them for classification, and OOD observations can be
//! promoted into new catalog entries without changing the storage schema.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use context_graph_mejepa_cf::{
    CF_MEJEPA_FAILURE_FINGERPRINTS, CF_MEJEPA_FINGERPRINT_AUDIT, CF_MEJEPA_FINGERPRINT_CALIBRATION,
    CF_MEJEPA_FINGERPRINT_DORMANCY, CF_MEJEPA_FINGERPRINT_FISHER, CF_MEJEPA_FINGERPRINT_REFERENCES,
    CF_MEJEPA_FINGERPRINT_REVERSE_INDEX,
};
use context_graph_mejepa_instruments::ExceptionClass;
use rocksdb::{WriteOptions, DB};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::calibration::cf;
use crate::error::MejepaInferError;
use crate::eval::MutationCategory;
use crate::types::{ChunkId, EmbedderId, FailureModeClass, OracleOutcome, TaskId, Verdict};

pub const FAILURE_FINGERPRINT_SCHEMA_VERSION: u32 = 1;
pub const SEED_KNOWN_GOOD_FINGERPRINTS: usize = 13;
pub const SEED_KNOWN_BAD_FINGERPRINTS: usize = 84;
pub const SEED_FAILURE_FINGERPRINTS: usize =
    SEED_KNOWN_GOOD_FINGERPRINTS + SEED_KNOWN_BAD_FINGERPRINTS;
pub const FORWARD_CACHE_EMBEDDER_COUNT: usize = 12;
pub const DEFAULT_FINGERPRINT_MATCH_TOP_K: usize = 3;
const MAX_TEXT_BYTES: usize = 512;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
#[serde(transparent)]
pub struct FingerprintId(pub [u8; 32]);

impl FingerprintId {
    pub fn from_canonical_parts(parts: &[&str]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(b"MEJEPA_FAILURE_FINGERPRINT_V1");
        for part in parts {
            hasher.update((part.len() as u64).to_be_bytes());
            hasher.update(part.as_bytes());
        }
        let digest = hasher.finalize();
        let mut out = [0u8; 32];
        out.copy_from_slice(&digest);
        Self(out)
    }

    pub fn hex(self) -> String {
        hex::encode(self.0)
    }

    pub fn key_bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FingerprintKind {
    KnownGood {
        repo: Option<String>,
        gold_patch_count: usize,
    },
    KnownBad {
        repo: String,
        mutation_category: MutationCategory,
        failure_mode: FailureModeClass,
        exception_class: Option<ExceptionClass>,
    },
    Unknown {
        observed_at_unix_ms: i64,
        observed_by_session: [u8; 16],
        ood_score: f32,
        embedder_disagreement_score: f32,
        active_learning_priority: u8,
    },
}

impl FingerprintKind {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        match self {
            Self::KnownGood {
                repo,
                gold_patch_count,
            } => {
                if *gold_patch_count == 0 {
                    return invalid(
                        "fingerprint.kind.gold_patch_count",
                        "KnownGood fingerprints require at least one gold patch",
                    );
                }
                if let Some(repo) = repo {
                    validate_text("fingerprint.kind.repo", repo)?;
                }
            }
            Self::KnownBad {
                repo,
                mutation_category,
                ..
            } => {
                validate_text("fingerprint.kind.repo", repo)?;
                if *mutation_category == MutationCategory::KnownGood {
                    return invalid(
                        "fingerprint.kind.mutation_category",
                        "KnownBad fingerprints cannot use mutation_category=known_good",
                    );
                }
            }
            Self::Unknown {
                observed_at_unix_ms,
                observed_by_session,
                ood_score,
                embedder_disagreement_score,
                active_learning_priority,
            } => {
                if *observed_at_unix_ms <= 0 {
                    return invalid(
                        "fingerprint.kind.observed_at_unix_ms",
                        "Unknown fingerprints require a positive observation timestamp",
                    );
                }
                if observed_by_session.iter().all(|b| *b == 0) {
                    return invalid(
                        "fingerprint.kind.observed_by_session",
                        "Unknown fingerprints require a non-zero session id",
                    );
                }
                validate_nonnegative_finite("fingerprint.kind.ood_score", *ood_score)?;
                validate_nonnegative_finite(
                    "fingerprint.kind.embedder_disagreement_score",
                    *embedder_disagreement_score,
                )?;
                if !(1..=5).contains(active_learning_priority) {
                    return invalid(
                        "fingerprint.kind.active_learning_priority",
                        "priority must be in 1..=5",
                    );
                }
            }
        }
        Ok(())
    }

    pub fn class_slug(&self) -> String {
        match self {
            Self::KnownGood { repo, .. } => match repo {
                Some(repo) => format!("known_good:{repo}"),
                None => "known_good:global".to_string(),
            },
            Self::KnownBad {
                repo,
                mutation_category,
                failure_mode,
                exception_class,
            } => {
                let exception = exception_class
                    .as_ref()
                    .map(ExceptionClass::slug)
                    .unwrap_or("no_exception");
                format!(
                    "known_bad:{repo}:{}:{}:{exception}",
                    mutation_category.slug(),
                    failure_mode.slug()
                )
            }
            Self::Unknown {
                observed_at_unix_ms,
                observed_by_session,
                ..
            } => format!(
                "unknown:{}:{}",
                observed_at_unix_ms,
                hex::encode(observed_by_session)
            ),
        }
    }

    pub fn kind_slug(&self) -> &'static str {
        match self {
            Self::KnownGood { .. } => "known_good",
            Self::KnownBad { .. } => "known_bad",
            Self::Unknown { .. } => "unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FingerprintCalibrationState {
    Uncalibrated,
    Calibrated,
    NeedsRecalibration,
}

impl FingerprintCalibrationState {
    pub fn slug(self) -> &'static str {
        match self {
            Self::Uncalibrated => "uncalibrated",
            Self::Calibrated => "calibrated",
            Self::NeedsRecalibration => "needs_recalibration",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FingerprintConfidence {
    pub classification_accuracy: Option<f32>,
    pub classification_precision: Option<f32>,
    pub unknown_recall: Option<f32>,
    pub calibration_observations: usize,
    pub calibration_state: FingerprintCalibrationState,
}

impl FingerprintConfidence {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        validate_optional_probability(
            "fingerprint.confidence.classification_accuracy",
            self.classification_accuracy,
        )?;
        validate_optional_probability(
            "fingerprint.confidence.classification_precision",
            self.classification_precision,
        )?;
        validate_optional_probability(
            "fingerprint.confidence.unknown_recall",
            self.unknown_recall,
        )?;
        if self.calibration_state == FingerprintCalibrationState::Calibrated
            && self.calibration_observations == 0
        {
            return invalid(
                "fingerprint.confidence.calibration_observations",
                "calibrated fingerprints require at least one calibration observation",
            );
        }
        Ok(())
    }
}

impl Default for FingerprintConfidence {
    fn default() -> Self {
        Self {
            classification_accuracy: None,
            classification_precision: None,
            unknown_recall: None,
            calibration_observations: 0,
            calibration_state: FingerprintCalibrationState::Uncalibrated,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FingerprintPairMetric {
    pub left_embedder: EmbedderId,
    pub right_embedder: EmbedderId,
    pub value: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FailureShapeFingerprint {
    pub schema_version: u32,
    pub fingerprint_id: FingerprintId,
    pub kind: FingerprintKind,
    pub name: String,
    pub source_corpus: String,
    pub source_manifest_sha256: Option<[u8; 32]>,
    pub centroid_by_embedder: BTreeMap<EmbedderId, Vec<f32>>,
    pub variance_by_embedder: BTreeMap<EmbedderId, f32>,
    pub tau_by_embedder: BTreeMap<EmbedderId, f32>,
    pub pairwise_cosine: Vec<FingerprintPairMetric>,
    pub pairwise_mutual_information: Vec<FingerprintPairMetric>,
    pub reference_chunks: Vec<ChunkId>,
    pub n_references: usize,
    pub oracle_outcome: Option<OracleOutcome>,
    pub is_canonical: bool,
    pub frozen_at_unix_ms: i64,
    pub confidence: FingerprintConfidence,
}

impl FailureShapeFingerprint {
    pub fn canonical_id(
        kind: &FingerprintKind,
        source_corpus: &str,
    ) -> Result<FingerprintId, MejepaInferError> {
        validate_text("fingerprint.source_corpus", source_corpus)?;
        kind.validate()?;
        Ok(FingerprintId::from_canonical_parts(&[
            source_corpus,
            &kind.class_slug(),
        ]))
    }

    pub fn validate(&self) -> Result<(), MejepaInferError> {
        if self.schema_version != FAILURE_FINGERPRINT_SCHEMA_VERSION {
            return invalid(
                "fingerprint.schema_version",
                &format!(
                    "expected schema version {FAILURE_FINGERPRINT_SCHEMA_VERSION}; got {}",
                    self.schema_version
                ),
            );
        }
        self.kind.validate()?;
        validate_text("fingerprint.name", &self.name)?;
        validate_text("fingerprint.source_corpus", &self.source_corpus)?;
        if self.frozen_at_unix_ms <= 0 {
            return invalid(
                "fingerprint.frozen_at_unix_ms",
                "fingerprint must have a positive freeze timestamp",
            );
        }
        if self.n_references == 0 {
            return invalid(
                "fingerprint.n_references",
                "fingerprint must be backed by at least one reference row",
            );
        }
        if self.reference_chunks.len() != self.n_references {
            return Err(MejepaInferError::DimMismatch {
                expected: self.n_references,
                actual: self.reference_chunks.len(),
                context: "fingerprint.reference_chunks".to_string(),
            });
        }
        for chunk_id in &self.reference_chunks {
            validate_text("fingerprint.reference_chunks", &chunk_id.0)?;
        }
        match (&self.kind, self.oracle_outcome) {
            (FingerprintKind::KnownGood { .. }, Some(OracleOutcome::Pass)) => {}
            (FingerprintKind::KnownGood { .. }, _) => {
                return invalid(
                    "fingerprint.oracle_outcome",
                    "KnownGood fingerprints require oracle_outcome=Pass",
                );
            }
            (FingerprintKind::KnownBad { .. }, Some(OracleOutcome::Fail)) => {}
            (FingerprintKind::KnownBad { .. }, _) => {
                return invalid(
                    "fingerprint.oracle_outcome",
                    "KnownBad fingerprints require oracle_outcome=Fail",
                );
            }
            (FingerprintKind::Unknown { .. }, None) => {}
            (FingerprintKind::Unknown { .. }, Some(_)) => {
                return invalid(
                    "fingerprint.oracle_outcome",
                    "Unknown fingerprints must not pin oracle_outcome until labeled",
                );
            }
        }
        if self.centroid_by_embedder.is_empty() {
            return invalid(
                "fingerprint.centroid_by_embedder",
                "fingerprint must contain at least one embedder centroid",
            );
        }
        validate_embedder_vector_map(
            "fingerprint.centroid_by_embedder",
            &self.centroid_by_embedder,
        )?;
        validate_embedder_scalar_map(
            "fingerprint.variance_by_embedder",
            &self.variance_by_embedder,
            &self.centroid_by_embedder,
            ScalarKind::NonNegative,
        )?;
        validate_embedder_scalar_map(
            "fingerprint.tau_by_embedder",
            &self.tau_by_embedder,
            &self.centroid_by_embedder,
            ScalarKind::Cosine,
        )?;
        validate_pair_metrics(
            "fingerprint.pairwise_cosine",
            &self.pairwise_cosine,
            &self.centroid_by_embedder,
            ScalarKind::Cosine,
        )?;
        validate_pair_metrics(
            "fingerprint.pairwise_mutual_information",
            &self.pairwise_mutual_information,
            &self.centroid_by_embedder,
            ScalarKind::NonNegative,
        )?;
        self.confidence.validate()?;
        let expected = Self::canonical_id(&self.kind, &self.source_corpus)?;
        if expected != self.fingerprint_id {
            return invalid(
                "fingerprint.fingerprint_id",
                &format!(
                    "does not match canonical id for kind/source; expected {} got {}",
                    expected.hex(),
                    self.fingerprint_id.hex()
                ),
            );
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FingerprintClassifierConfig {
    pub require_canonical: bool,
    pub require_calibrated: bool,
    pub top_k: usize,
}

impl Default for FingerprintClassifierConfig {
    fn default() -> Self {
        Self {
            require_canonical: true,
            require_calibrated: true,
            top_k: DEFAULT_FINGERPRINT_MATCH_TOP_K,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FingerprintDecisionReason {
    KnownGoodOnly,
    KnownBadOnly,
    KnownGoodKnownBadConflict,
    NoKnownMatch,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FingerprintEmbedderScore {
    pub embedder: EmbedderId,
    pub cosine: f32,
    pub tau: f32,
    pub margin: f32,
    pub matched: bool,
}

impl FingerprintEmbedderScore {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        validate_text("fingerprint_embedder_score.embedder", &self.embedder.0)?;
        validate_cosine("fingerprint_embedder_score.cosine", self.cosine)?;
        validate_cosine("fingerprint_embedder_score.tau", self.tau)?;
        validate_finite("fingerprint_embedder_score.margin", self.margin)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FingerprintCandidateScore {
    pub fingerprint_id: FingerprintId,
    pub name: String,
    pub kind: FingerprintKind,
    pub oracle_outcome: Option<OracleOutcome>,
    pub matched: bool,
    pub mean_cosine: f32,
    pub min_margin: f32,
    pub embedder_scores: Vec<FingerprintEmbedderScore>,
    pub n_references: usize,
    pub calibration_state: FingerprintCalibrationState,
}

impl FingerprintCandidateScore {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        if self.fingerprint_id == FingerprintId::default() {
            return invalid(
                "fingerprint_candidate_score.fingerprint_id",
                "candidate fingerprint id must be non-zero",
            );
        }
        validate_text("fingerprint_candidate_score.name", &self.name)?;
        self.kind.validate()?;
        validate_cosine("fingerprint_candidate_score.mean_cosine", self.mean_cosine)?;
        validate_finite("fingerprint_candidate_score.min_margin", self.min_margin)?;
        if self.embedder_scores.is_empty() {
            return invalid(
                "fingerprint_candidate_score.embedder_scores",
                "candidate score requires per-embedder scores",
            );
        }
        for score in &self.embedder_scores {
            score.validate()?;
        }
        if self.matched != self.embedder_scores.iter().all(|score| score.matched) {
            return invalid(
                "fingerprint_candidate_score.matched",
                "candidate matched flag must equal all per-embedder Gtau checks",
            );
        }
        if self.n_references == 0 {
            return invalid(
                "fingerprint_candidate_score.n_references",
                "candidate score requires at least one reference",
            );
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FingerprintClassification {
    pub verdict: Verdict,
    pub reason: FingerprintDecisionReason,
    pub primary_match: Option<FingerprintCandidateScore>,
    pub ranked_matches: Vec<FingerprintCandidateScore>,
    pub matched_known_good_count: usize,
    pub matched_known_bad_count: usize,
    pub matched_unknown_count: usize,
    pub scored_fingerprint_count: usize,
}

impl FingerprintClassification {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        if self.scored_fingerprint_count == 0 {
            return invalid(
                "fingerprint_classification.scored_fingerprint_count",
                "classifier must score at least one fingerprint",
            );
        }
        if let Some(primary) = &self.primary_match {
            primary.validate()?;
            if !primary.matched {
                return invalid(
                    "fingerprint_classification.primary_match",
                    "primary match must satisfy Gtau",
                );
            }
        }
        for candidate in &self.ranked_matches {
            candidate.validate()?;
        }
        let ranked_matched_known_good_count = self
            .ranked_matches
            .iter()
            .filter(|candidate| {
                candidate.matched && matches!(&candidate.kind, FingerprintKind::KnownGood { .. })
            })
            .count();
        let ranked_matched_known_bad_count = self
            .ranked_matches
            .iter()
            .filter(|candidate| {
                candidate.matched && matches!(&candidate.kind, FingerprintKind::KnownBad { .. })
            })
            .count();
        if ranked_matched_known_good_count > self.matched_known_good_count {
            return invalid(
                "fingerprint_classification.matched_known_good_count",
                "ranked matches contain more KnownGood hits than the aggregate count",
            );
        }
        if ranked_matched_known_bad_count > self.matched_known_bad_count {
            return invalid(
                "fingerprint_classification.matched_known_bad_count",
                "ranked matches contain more KnownBad hits than the aggregate count",
            );
        }
        match self.reason {
            FingerprintDecisionReason::KnownGoodOnly => {
                if self.verdict != Verdict::Pass
                    || self.primary_match.is_none()
                    || self.matched_known_good_count == 0
                    || self.matched_known_bad_count != 0
                {
                    return invalid(
                        "fingerprint_classification.reason",
                        "KnownGoodOnly requires pass verdict and no KnownBad matches",
                    );
                }
            }
            FingerprintDecisionReason::KnownBadOnly => {
                if self.verdict != Verdict::Fail
                    || self.primary_match.is_none()
                    || self.matched_known_bad_count == 0
                    || self.matched_known_good_count != 0
                {
                    return invalid(
                        "fingerprint_classification.reason",
                        "KnownBadOnly requires fail verdict and no KnownGood matches",
                    );
                }
            }
            FingerprintDecisionReason::KnownGoodKnownBadConflict => {
                if self.verdict != Verdict::Abstain
                    || self.primary_match.is_none()
                    || self.matched_known_good_count == 0
                    || self.matched_known_bad_count == 0
                {
                    return invalid(
                        "fingerprint_classification.reason",
                        "conflict requires abstain verdict and both KnownGood/KnownBad matches",
                    );
                }
            }
            FingerprintDecisionReason::NoKnownMatch => {
                if self.verdict != Verdict::OutOfDistribution
                    || self.primary_match.is_some()
                    || self.matched_known_good_count != 0
                    || self.matched_known_bad_count != 0
                {
                    return invalid(
                        "fingerprint_classification.reason",
                        "NoKnownMatch requires OOD verdict and no known primary match",
                    );
                }
            }
        }
        Ok(())
    }
}

pub fn classify_failure_fingerprint_observation(
    catalog: &[FailureShapeFingerprint],
    observation_by_embedder: &BTreeMap<EmbedderId, Vec<f32>>,
    config: FingerprintClassifierConfig,
) -> Result<FingerprintClassification, MejepaInferError> {
    if catalog.is_empty() {
        return invalid(
            "fingerprint_classifier.catalog",
            "catalog must contain at least one fingerprint",
        );
    }
    if config.top_k == 0 {
        return invalid("fingerprint_classifier.top_k", "top_k must be >= 1");
    }
    validate_embedder_vector_map(
        "fingerprint_classifier.observation_by_embedder",
        observation_by_embedder,
    )?;

    let mut candidates = Vec::new();
    for fingerprint in catalog {
        if !fingerprint_is_gate_eligible(fingerprint, config)? {
            continue;
        }
        candidates.push(score_fingerprint_candidate(
            fingerprint,
            observation_by_embedder,
        )?);
    }
    if candidates.is_empty() {
        return invalid(
            "fingerprint_classifier.catalog",
            "catalog contained no canonical calibrated fingerprints eligible for Gtau classification",
        );
    }

    candidates.sort_by(compare_candidate_rank);
    let matched_known_good_count = candidates
        .iter()
        .filter(|candidate| {
            candidate.matched && matches!(&candidate.kind, FingerprintKind::KnownGood { .. })
        })
        .count();
    let matched_known_bad_count = candidates
        .iter()
        .filter(|candidate| {
            candidate.matched && matches!(&candidate.kind, FingerprintKind::KnownBad { .. })
        })
        .count();
    let matched_unknown_count = candidates
        .iter()
        .filter(|candidate| {
            candidate.matched && matches!(&candidate.kind, FingerprintKind::Unknown { .. })
        })
        .count();

    let (verdict, reason, primary_match) =
        match (matched_known_good_count > 0, matched_known_bad_count > 0) {
            (true, false) => (
                Verdict::Pass,
                FingerprintDecisionReason::KnownGoodOnly,
                candidates
                    .iter()
                    .find(|candidate| {
                        candidate.matched
                            && matches!(&candidate.kind, FingerprintKind::KnownGood { .. })
                    })
                    .cloned(),
            ),
            (false, true) => (
                Verdict::Fail,
                FingerprintDecisionReason::KnownBadOnly,
                candidates
                    .iter()
                    .find(|candidate| {
                        candidate.matched
                            && matches!(&candidate.kind, FingerprintKind::KnownBad { .. })
                    })
                    .cloned(),
            ),
            (true, true) => (
                Verdict::Abstain,
                FingerprintDecisionReason::KnownGoodKnownBadConflict,
                candidates
                    .iter()
                    .find(|candidate| candidate.matched)
                    .cloned(),
            ),
            (false, false) => (
                Verdict::OutOfDistribution,
                FingerprintDecisionReason::NoKnownMatch,
                None,
            ),
        };

    let ranked_matches = candidates
        .iter()
        .take(config.top_k)
        .cloned()
        .collect::<Vec<_>>();
    let classification = FingerprintClassification {
        verdict,
        reason,
        primary_match,
        ranked_matches,
        matched_known_good_count,
        matched_known_bad_count,
        matched_unknown_count,
        scored_fingerprint_count: candidates.len(),
    };
    classification.validate()?;
    Ok(classification)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FingerprintReference {
    pub fingerprint_id: FingerprintId,
    pub reference_id: String,
    pub task_id: TaskId,
    pub repo: String,
    pub mutation_category: MutationCategory,
    pub chunk_id: ChunkId,
    pub embedder_ids: Vec<EmbedderId>,
    pub oracle_outcome: OracleOutcome,
    pub witness_hash: [u8; 32],
    pub source_manifest_sha256: [u8; 32],
}

impl FingerprintReference {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        validate_text("fingerprint_reference.reference_id", &self.reference_id)?;
        validate_text("fingerprint_reference.task_id", &self.task_id.0)?;
        validate_text("fingerprint_reference.repo", &self.repo)?;
        validate_text("fingerprint_reference.chunk_id", &self.chunk_id.0)?;
        validate_embedder_id_list("fingerprint_reference.embedder_ids", &self.embedder_ids)?;
        validate_nonzero_sha("fingerprint_reference.witness_hash", &self.witness_hash)?;
        validate_nonzero_sha(
            "fingerprint_reference.source_manifest_sha256",
            &self.source_manifest_sha256,
        )?;
        if self.mutation_category == MutationCategory::KnownGood
            && self.oracle_outcome != OracleOutcome::Pass
        {
            return invalid(
                "fingerprint_reference.oracle_outcome",
                "known_good references must carry a Pass oracle outcome",
            );
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FingerprintReferenceLocator {
    pub fingerprint_id: FingerprintId,
    pub reference_id: String,
}

impl FingerprintReferenceLocator {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        validate_text(
            "fingerprint_reference_locator.reference_id",
            &self.reference_id,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FingerprintCalibrationRecord {
    pub fingerprint_id: FingerprintId,
    pub calibrated_at_unix_ms: i64,
    pub tau_by_embedder: BTreeMap<EmbedderId, f32>,
    pub same_session_band_percentile: f32,
    pub sample_count: usize,
}

impl FingerprintCalibrationRecord {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        if self.calibrated_at_unix_ms <= 0 {
            return invalid(
                "fingerprint_calibration.calibrated_at_unix_ms",
                "calibration timestamp must be positive",
            );
        }
        if self.sample_count == 0 {
            return invalid(
                "fingerprint_calibration.sample_count",
                "calibration requires at least one sample",
            );
        }
        validate_probability(
            "fingerprint_calibration.same_session_band_percentile",
            self.same_session_band_percentile,
        )?;
        for (embedder, tau) in &self.tau_by_embedder {
            validate_text("fingerprint_calibration.embedder_id", &embedder.0)?;
            validate_cosine("fingerprint_calibration.tau_by_embedder", *tau)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FingerprintAuditAction {
    Created,
    Updated,
    Calibrated,
    PromotedCanonical,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FingerprintAuditEntry {
    pub fingerprint_id: FingerprintId,
    pub action: FingerprintAuditAction,
    pub actor: String,
    pub created_at_unix_ms: i64,
    pub detail: String,
}

impl FingerprintAuditEntry {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        validate_text("fingerprint_audit.actor", &self.actor)?;
        validate_text("fingerprint_audit.detail", &self.detail)?;
        if self.created_at_unix_ms <= 0 {
            return invalid(
                "fingerprint_audit.created_at_unix_ms",
                "audit timestamp must be positive",
            );
        }
        Ok(())
    }
}

pub fn fingerprint_key(id: FingerprintId) -> Vec<u8> {
    id.key_bytes().to_vec()
}

pub fn fingerprint_reference_key(
    fingerprint_id: FingerprintId,
    reference_id: &str,
) -> Result<Vec<u8>, MejepaInferError> {
    validate_text("fingerprint_reference.reference_id", reference_id)?;
    bincode::serialize(&(fingerprint_id, reference_id)).map_err(Into::into)
}

pub fn fingerprint_reverse_index_key(
    chunk_id: &ChunkId,
    fingerprint_id: FingerprintId,
    reference_id: &str,
) -> Result<Vec<u8>, MejepaInferError> {
    validate_text("fingerprint_reverse_index.chunk_id", &chunk_id.0)?;
    validate_text("fingerprint_reverse_index.reference_id", reference_id)?;
    bincode::serialize(&(chunk_id, fingerprint_id, reference_id)).map_err(Into::into)
}

pub fn fingerprint_calibration_key(
    fingerprint_id: FingerprintId,
    calibrated_at_unix_ms: i64,
) -> Result<Vec<u8>, MejepaInferError> {
    if calibrated_at_unix_ms <= 0 {
        return invalid(
            "fingerprint_calibration.calibrated_at_unix_ms",
            "calibration timestamp must be positive",
        );
    }
    bincode::serialize(&(fingerprint_id, calibrated_at_unix_ms)).map_err(Into::into)
}

pub fn fingerprint_audit_key(
    fingerprint_id: FingerprintId,
    created_at_unix_ms: i64,
    actor: &str,
) -> Result<Vec<u8>, MejepaInferError> {
    validate_text("fingerprint_audit.actor", actor)?;
    if created_at_unix_ms <= 0 {
        return invalid(
            "fingerprint_audit.created_at_unix_ms",
            "audit timestamp must be positive",
        );
    }
    bincode::serialize(&(fingerprint_id, created_at_unix_ms, actor)).map_err(Into::into)
}

pub fn write_fingerprint_sync_readback(
    db: &DB,
    fingerprint: &FailureShapeFingerprint,
) -> Result<(), MejepaInferError> {
    fingerprint.validate()?;
    write_value_sync_readback(
        db,
        CF_MEJEPA_FAILURE_FINGERPRINTS,
        &fingerprint_key(fingerprint.fingerprint_id),
        fingerprint,
    )
}

pub fn read_fingerprint(
    db: &DB,
    id: FingerprintId,
) -> Result<Option<FailureShapeFingerprint>, MejepaInferError> {
    read_value(db, CF_MEJEPA_FAILURE_FINGERPRINTS, &fingerprint_key(id))
}

pub fn write_fingerprint_reference_sync_readback(
    db: &DB,
    reference: &FingerprintReference,
) -> Result<(), MejepaInferError> {
    reference.validate()?;
    write_value_sync_readback(
        db,
        CF_MEJEPA_FINGERPRINT_REFERENCES,
        &fingerprint_reference_key(reference.fingerprint_id, &reference.reference_id)?,
        reference,
    )?;
    let locator = FingerprintReferenceLocator {
        fingerprint_id: reference.fingerprint_id,
        reference_id: reference.reference_id.clone(),
    };
    write_value_sync_readback(
        db,
        CF_MEJEPA_FINGERPRINT_REVERSE_INDEX,
        &fingerprint_reverse_index_key(
            &reference.chunk_id,
            reference.fingerprint_id,
            &reference.reference_id,
        )?,
        &locator,
    )
}

pub fn read_fingerprint_reference(
    db: &DB,
    fingerprint_id: FingerprintId,
    reference_id: &str,
) -> Result<Option<FingerprintReference>, MejepaInferError> {
    read_value(
        db,
        CF_MEJEPA_FINGERPRINT_REFERENCES,
        &fingerprint_reference_key(fingerprint_id, reference_id)?,
    )
}

pub fn write_fingerprint_calibration_sync_readback(
    db: &DB,
    record: &FingerprintCalibrationRecord,
) -> Result<(), MejepaInferError> {
    record.validate()?;
    write_value_sync_readback(
        db,
        CF_MEJEPA_FINGERPRINT_CALIBRATION,
        &fingerprint_calibration_key(record.fingerprint_id, record.calibrated_at_unix_ms)?,
        record,
    )
}

pub fn write_fingerprint_audit_sync_readback(
    db: &DB,
    entry: &FingerprintAuditEntry,
) -> Result<(), MejepaInferError> {
    entry.validate()?;
    write_value_sync_readback(
        db,
        CF_MEJEPA_FINGERPRINT_AUDIT,
        &fingerprint_audit_key(entry.fingerprint_id, entry.created_at_unix_ms, &entry.actor)?,
        entry,
    )
}

// ---------------------------------------------------------------------------
// TASK-FP-008 (#317) — per-fingerprint EWC Fisher + CBP dormancy snapshots.
// ---------------------------------------------------------------------------
//
// These two record types are how catalog stability under continual learning is
// enforced. Each snapshot binds to a `FingerprintId` and records, per predictor
// output dimension, either the EWC Fisher diagonal weight at last calibration
// (Kirkpatrick 2017) or the CBP utility EMA (Dohare/Sutton/Mahmood 2024).
// The training loop later reads them to apply a quadratic penalty (EWC) and a
// dormant-feature reset (CBP) so the catalog's per-fingerprint accuracy on the
// seed 97-row corpus does not drift as fresh Unknown fingerprints are
// promoted.

pub const DEFAULT_FINGERPRINT_EWC_LAMBDA: f32 = 2_000.0;
pub const DEFAULT_FINGERPRINT_CBP_DORMANCY_THRESHOLD: f32 = 0.05;
pub const FINGERPRINT_DORMANCY_RESET_CODE: &str = "MEJEPA_FINGERPRINT_DORMANCY_RESET";

/// Per-fingerprint EWC Fisher diagonal at last calibration.
///
/// `fisher_by_dimension` is keyed by predictor-output dimension index (not by
/// embedder slot). Values must be finite and non-negative; an all-zero map is
/// rejected at validation time because a Fisher diagonal with no signal cannot
/// drive an EWC penalty and would silently disable consolidation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FingerprintFisherSnapshot {
    pub fingerprint_id: FingerprintId,
    pub calibrated_at_unix_ms: i64,
    pub sample_count: usize,
    pub theta_star_by_dimension: BTreeMap<u32, f32>,
    pub fisher_by_dimension: BTreeMap<u32, f32>,
}

impl FingerprintFisherSnapshot {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        if self.calibrated_at_unix_ms <= 0 {
            return invalid(
                "fingerprint_fisher.calibrated_at_unix_ms",
                "fisher snapshot must have a positive calibration timestamp",
            );
        }
        if self.sample_count == 0 {
            return invalid(
                "fingerprint_fisher.sample_count",
                "fisher snapshot requires at least one sample",
            );
        }
        if self.fisher_by_dimension.is_empty() {
            return invalid(
                "fingerprint_fisher.fisher_by_dimension",
                "fisher snapshot must cover at least one predictor dimension",
            );
        }
        if self.theta_star_by_dimension.is_empty() {
            return invalid(
                "fingerprint_fisher.theta_star_by_dimension",
                "fisher snapshot must anchor at least one predictor dimension",
            );
        }
        if self.theta_star_by_dimension.keys().collect::<Vec<_>>()
            != self.fisher_by_dimension.keys().collect::<Vec<_>>()
        {
            return invalid(
                "fingerprint_fisher.dimensions",
                "theta_star_by_dimension and fisher_by_dimension must cover the same dimensions",
            );
        }
        for (dim, value) in &self.theta_star_by_dimension {
            if !value.is_finite() {
                return invalid(
                    "fingerprint_fisher.theta_star_by_dimension",
                    &format!("theta_star value for dim {dim} is not finite"),
                );
            }
        }
        let mut all_zero = true;
        for (dim, value) in &self.fisher_by_dimension {
            if !value.is_finite() {
                return invalid(
                    "fingerprint_fisher.fisher_by_dimension",
                    &format!("fisher value for dim {dim} is not finite"),
                );
            }
            if *value < 0.0 {
                return invalid(
                    "fingerprint_fisher.fisher_by_dimension",
                    &format!("fisher value for dim {dim} must be >= 0"),
                );
            }
            if *value > 0.0 {
                all_zero = false;
            }
        }
        if all_zero {
            return invalid(
                "fingerprint_fisher.fisher_by_dimension",
                "fisher snapshot rejected: all dimensions zero would silently disable the EWC penalty",
            );
        }
        Ok(())
    }
}

/// Per-fingerprint CBP dormancy EMA snapshot.
///
/// `dormancy_ema_by_dimension` is a rolling utility score in `[0.0, 1.0]`
/// per predictor dimension. Values approaching zero mark "dormant" features
/// that the CBP reset should re-randomize; high values mark load-bearing
/// dimensions that must be preserved. NaN values must be coerced to 1.0
/// before persistence (preserve-by-default) and a `dormancy_reset_count`
/// counter incremented; the record carries that counter for audit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FingerprintDormancySnapshot {
    pub fingerprint_id: FingerprintId,
    pub recorded_at_unix_ms: i64,
    pub window_steps: u64,
    pub dormancy_ema_by_dimension: BTreeMap<u32, f32>,
    pub dormancy_reset_count: u32,
}

impl FingerprintDormancySnapshot {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        if self.recorded_at_unix_ms <= 0 {
            return invalid(
                "fingerprint_dormancy.recorded_at_unix_ms",
                "dormancy snapshot must have a positive timestamp",
            );
        }
        if self.window_steps == 0 {
            return invalid(
                "fingerprint_dormancy.window_steps",
                "dormancy snapshot must cover at least one training step",
            );
        }
        if self.dormancy_ema_by_dimension.is_empty() {
            return invalid(
                "fingerprint_dormancy.dormancy_ema_by_dimension",
                "dormancy snapshot must cover at least one predictor dimension",
            );
        }
        for (dim, value) in &self.dormancy_ema_by_dimension {
            if !value.is_finite() {
                return invalid(
                    "fingerprint_dormancy.dormancy_ema_by_dimension",
                    &format!(
                        "dormancy EMA for dim {dim} is not finite; caller must coerce NaN to 1.0 and bump reset_count"
                    ),
                );
            }
            if *value < 0.0 || *value > 1.0 {
                return invalid(
                    "fingerprint_dormancy.dormancy_ema_by_dimension",
                    &format!("dormancy EMA for dim {dim} must lie in [0.0, 1.0]"),
                );
            }
        }
        Ok(())
    }
}

pub fn fingerprint_fisher_key(id: FingerprintId) -> Vec<u8> {
    id.key_bytes().to_vec()
}

pub fn fingerprint_dormancy_key(id: FingerprintId) -> Vec<u8> {
    id.key_bytes().to_vec()
}

pub fn write_fingerprint_fisher_sync_readback(
    db: &DB,
    snapshot: &FingerprintFisherSnapshot,
) -> Result<(), MejepaInferError> {
    snapshot.validate()?;
    write_value_sync_readback(
        db,
        CF_MEJEPA_FINGERPRINT_FISHER,
        &fingerprint_fisher_key(snapshot.fingerprint_id),
        snapshot,
    )
}

pub fn read_fingerprint_fisher(
    db: &DB,
    id: FingerprintId,
) -> Result<Option<FingerprintFisherSnapshot>, MejepaInferError> {
    read_value(
        db,
        CF_MEJEPA_FINGERPRINT_FISHER,
        &fingerprint_fisher_key(id),
    )
}

pub fn write_fingerprint_dormancy_sync_readback(
    db: &DB,
    snapshot: &FingerprintDormancySnapshot,
) -> Result<(), MejepaInferError> {
    snapshot.validate()?;
    write_value_sync_readback(
        db,
        CF_MEJEPA_FINGERPRINT_DORMANCY,
        &fingerprint_dormancy_key(snapshot.fingerprint_id),
        snapshot,
    )
}

pub fn read_fingerprint_dormancy(
    db: &DB,
    id: FingerprintId,
) -> Result<Option<FingerprintDormancySnapshot>, MejepaInferError> {
    read_value(
        db,
        CF_MEJEPA_FINGERPRINT_DORMANCY,
        &fingerprint_dormancy_key(id),
    )
}

pub fn require_fingerprint_fisher(
    db: &DB,
    id: FingerprintId,
) -> Result<FingerprintFisherSnapshot, MejepaInferError> {
    read_fingerprint_fisher(db, id)?.ok_or_else(|| MejepaInferError::FingerprintFisherMissing {
        fingerprint_id: id.hex(),
    })
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FingerprintEwcPenalty {
    pub fingerprint_id: FingerprintId,
    pub lambda: f32,
    pub parameter_count: usize,
    pub active_dimension_count: usize,
    pub raw_quadratic_penalty: f32,
    pub scaled_penalty: f32,
}

impl FingerprintEwcPenalty {
    pub fn compute(
        predictor_params: &[f32],
        snapshot: &FingerprintFisherSnapshot,
        lambda: f32,
    ) -> Result<Self, MejepaInferError> {
        snapshot.validate()?;
        if predictor_params.is_empty() {
            return invalid(
                "fingerprint_ewc.predictor_params",
                "predictor parameter vector must be non-empty",
            );
        }
        if !lambda.is_finite() || lambda < 0.0 {
            return invalid(
                "fingerprint_ewc.lambda",
                "lambda must be finite and non-negative",
            );
        }
        for (idx, value) in predictor_params.iter().enumerate() {
            validate_finite(&format!("fingerprint_ewc.predictor_params[{idx}]"), *value)?;
        }
        let max_dim = *snapshot
            .fisher_by_dimension
            .keys()
            .next_back()
            .expect("snapshot validation ensures at least one dimension")
            as usize;
        if max_dim >= predictor_params.len() {
            return Err(MejepaInferError::DimMismatch {
                expected: max_dim + 1,
                actual: predictor_params.len(),
                context: "fingerprint_ewc.predictor_params".to_string(),
            });
        }
        let mut raw = 0.0_f32;
        let mut active_dimension_count = 0_usize;
        for (dim, fisher) in &snapshot.fisher_by_dimension {
            let theta_star = snapshot.theta_star_by_dimension.get(dim).ok_or_else(|| {
                MejepaInferError::InvalidInput {
                    field: "fingerprint_ewc.theta_star_by_dimension".to_string(),
                    detail: format!("missing theta star for dim {dim}"),
                }
            })?;
            if *fisher > 0.0 {
                active_dimension_count += 1;
            }
            let theta = predictor_params[*dim as usize];
            raw += *fisher * (theta - *theta_star).powi(2);
        }
        validate_nonnegative_finite("fingerprint_ewc.raw_quadratic_penalty", raw)?;
        let scaled_penalty = 0.5 * lambda * raw;
        validate_nonnegative_finite("fingerprint_ewc.scaled_penalty", scaled_penalty)?;
        Ok(Self {
            fingerprint_id: snapshot.fingerprint_id,
            lambda,
            parameter_count: predictor_params.len(),
            active_dimension_count,
            raw_quadratic_penalty: raw,
            scaled_penalty,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FingerprintDormancySanitization {
    pub sanitized_ema_by_dimension: BTreeMap<u32, f32>,
    pub reset_dimensions: Vec<u32>,
    pub reset_count: u32,
    pub reset_code: Option<String>,
}

pub fn sanitize_fingerprint_dormancy_ema(
    raw_ema_by_dimension: &BTreeMap<u32, f32>,
) -> Result<FingerprintDormancySanitization, MejepaInferError> {
    if raw_ema_by_dimension.is_empty() {
        return invalid(
            "fingerprint_dormancy.raw_ema_by_dimension",
            "dormancy EMA input must cover at least one predictor dimension",
        );
    }
    let mut sanitized = BTreeMap::new();
    let mut reset_dimensions = Vec::new();
    for (dim, value) in raw_ema_by_dimension {
        let next = if value.is_finite() {
            if !(0.0..=1.0).contains(value) {
                return invalid(
                    "fingerprint_dormancy.raw_ema_by_dimension",
                    &format!("dormancy EMA for dim {dim} must lie in [0.0, 1.0]"),
                );
            }
            *value
        } else {
            reset_dimensions.push(*dim);
            1.0
        };
        sanitized.insert(*dim, next);
    }
    let reset_count = reset_dimensions.len() as u32;
    Ok(FingerprintDormancySanitization {
        sanitized_ema_by_dimension: sanitized,
        reset_dimensions,
        reset_count,
        reset_code: (reset_count > 0).then(|| FINGERPRINT_DORMANCY_RESET_CODE.to_string()),
    })
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FingerprintCbpResetPlan {
    pub fingerprint_id: FingerprintId,
    pub threshold: f32,
    pub reset_dimensions: Vec<u32>,
    pub protected_dimensions: Vec<u32>,
}

pub struct FingerprintCbpReset;

impl FingerprintCbpReset {
    pub fn should_reset(
        dim: u32,
        dormancy_ema: f32,
        threshold: f32,
    ) -> Result<bool, MejepaInferError> {
        validate_probability("fingerprint_cbp.dormancy_ema", dormancy_ema)?;
        validate_probability("fingerprint_cbp.threshold", threshold)?;
        let _ = dim;
        Ok(dormancy_ema <= threshold)
    }

    pub fn plan(
        snapshot: &FingerprintDormancySnapshot,
        threshold: f32,
    ) -> Result<FingerprintCbpResetPlan, MejepaInferError> {
        snapshot.validate()?;
        validate_probability("fingerprint_cbp.threshold", threshold)?;
        let mut reset_dimensions = Vec::new();
        let mut protected_dimensions = Vec::new();
        for (dim, dormancy_ema) in &snapshot.dormancy_ema_by_dimension {
            if Self::should_reset(*dim, *dormancy_ema, threshold)? {
                reset_dimensions.push(*dim);
            } else {
                protected_dimensions.push(*dim);
            }
        }
        Ok(FingerprintCbpResetPlan {
            fingerprint_id: snapshot.fingerprint_id,
            threshold,
            reset_dimensions,
            protected_dimensions,
        })
    }
}

fn write_value_sync_readback<T>(
    db: &DB,
    cf_name: &str,
    key: &[u8],
    value: &T,
) -> Result<(), MejepaInferError>
where
    T: Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug,
{
    let cf = cf(db, cf_name)?;
    let bytes = bincode::serialize(value)?;
    let mut opts = WriteOptions::default();
    opts.set_sync(true);
    db.put_cf_opt(cf, key, &bytes, &opts)?;
    db.flush_cf(cf)?;
    let readback = db
        .get_cf(cf, key)?
        .ok_or_else(|| MejepaInferError::InvalidInput {
            field: cf_name.to_string(),
            detail: "sync write readback returned no row".to_string(),
        })?;
    if readback != bytes {
        return Err(MejepaInferError::InvalidInput {
            field: cf_name.to_string(),
            detail: "sync write readback bytes differ from encoded input".to_string(),
        });
    }
    let decoded: T = bincode::deserialize(&readback)?;
    if decoded != *value {
        return Err(MejepaInferError::InvalidInput {
            field: cf_name.to_string(),
            detail: format!("sync write readback decoded value differs: {decoded:?}"),
        });
    }
    Ok(())
}

fn read_value<T>(db: &DB, cf_name: &str, key: &[u8]) -> Result<Option<T>, MejepaInferError>
where
    T: serde::de::DeserializeOwned,
{
    let cf = cf(db, cf_name)?;
    db.get_cf(cf, key)?
        .map(|bytes| bincode::deserialize(&bytes).map_err(Into::into))
        .transpose()
}

fn fingerprint_is_gate_eligible(
    fingerprint: &FailureShapeFingerprint,
    config: FingerprintClassifierConfig,
) -> Result<bool, MejepaInferError> {
    fingerprint.validate()?;
    if config.require_canonical && !fingerprint.is_canonical {
        return Ok(false);
    }
    if config.require_calibrated
        && fingerprint.confidence.calibration_state != FingerprintCalibrationState::Calibrated
    {
        return Ok(false);
    }
    Ok(true)
}

fn score_fingerprint_candidate(
    fingerprint: &FailureShapeFingerprint,
    observation_by_embedder: &BTreeMap<EmbedderId, Vec<f32>>,
) -> Result<FingerprintCandidateScore, MejepaInferError> {
    let mut embedder_scores = Vec::with_capacity(fingerprint.centroid_by_embedder.len());
    let mut cosine_sum = 0.0_f32;
    let mut min_margin = f32::INFINITY;
    for (embedder, centroid) in &fingerprint.centroid_by_embedder {
        let observation = observation_by_embedder.get(embedder).ok_or_else(|| {
            MejepaInferError::InvalidInput {
                field: "fingerprint_classifier.observation_by_embedder".to_string(),
                detail: format!("missing observation vector for embedder {}", embedder.0),
            }
        })?;
        let tau = *fingerprint.tau_by_embedder.get(embedder).ok_or_else(|| {
            MejepaInferError::InvalidInput {
                field: "fingerprint.tau_by_embedder".to_string(),
                detail: format!("missing tau for embedder {}", embedder.0),
            }
        })?;
        let cosine = cosine_similarity(observation, centroid, &embedder.0)?;
        let margin = cosine - tau;
        min_margin = min_margin.min(margin);
        cosine_sum += cosine;
        embedder_scores.push(FingerprintEmbedderScore {
            embedder: embedder.clone(),
            cosine,
            tau,
            margin,
            matched: cosine >= tau,
        });
    }
    let mean_cosine = cosine_sum / embedder_scores.len() as f32;
    let matched = embedder_scores.iter().all(|score| score.matched);
    let candidate = FingerprintCandidateScore {
        fingerprint_id: fingerprint.fingerprint_id,
        name: fingerprint.name.clone(),
        kind: fingerprint.kind.clone(),
        oracle_outcome: fingerprint.oracle_outcome,
        matched,
        mean_cosine,
        min_margin,
        embedder_scores,
        n_references: fingerprint.n_references,
        calibration_state: fingerprint.confidence.calibration_state,
    };
    candidate.validate()?;
    Ok(candidate)
}

fn cosine_similarity(left: &[f32], right: &[f32], context: &str) -> Result<f32, MejepaInferError> {
    if left.len() != right.len() {
        return Err(MejepaInferError::DimMismatch {
            expected: right.len(),
            actual: left.len(),
            context: format!("fingerprint_classifier.embedder[{context}]"),
        });
    }
    let mut dot = 0.0_f64;
    let mut left_norm = 0.0_f64;
    let mut right_norm = 0.0_f64;
    for (idx, (left_value, right_value)) in left.iter().zip(right.iter()).enumerate() {
        validate_finite(
            &format!("fingerprint_classifier.observation[{context}][{idx}]"),
            *left_value,
        )?;
        validate_finite(
            &format!("fingerprint_classifier.centroid[{context}][{idx}]"),
            *right_value,
        )?;
        let left_f64 = f64::from(*left_value);
        let right_f64 = f64::from(*right_value);
        dot += left_f64 * right_f64;
        left_norm += left_f64 * left_f64;
        right_norm += right_f64 * right_f64;
    }
    if left_norm <= f64::EPSILON || right_norm <= f64::EPSILON {
        return invalid(
            "fingerprint_classifier.cosine",
            "cosine input vectors must have non-zero norm",
        );
    }
    let cosine = (dot / (left_norm.sqrt() * right_norm.sqrt())).clamp(-1.0, 1.0) as f32;
    validate_cosine("fingerprint_classifier.cosine", cosine)?;
    Ok(cosine)
}

fn compare_candidate_rank(
    left: &FingerprintCandidateScore,
    right: &FingerprintCandidateScore,
) -> Ordering {
    right
        .matched
        .cmp(&left.matched)
        .then_with(|| {
            right
                .mean_cosine
                .partial_cmp(&left.mean_cosine)
                .unwrap_or(Ordering::Equal)
        })
        .then_with(|| {
            right
                .min_margin
                .partial_cmp(&left.min_margin)
                .unwrap_or(Ordering::Equal)
        })
        .then_with(|| {
            left.fingerprint_id
                .key_bytes()
                .cmp(&right.fingerprint_id.key_bytes())
        })
}

#[derive(Debug, Clone, Copy)]
enum ScalarKind {
    Cosine,
    NonNegative,
}

fn validate_embedder_vector_map(
    field: &str,
    vectors: &BTreeMap<EmbedderId, Vec<f32>>,
) -> Result<(), MejepaInferError> {
    for (embedder, vector) in vectors {
        validate_text(&format!("{field}.embedder_id"), &embedder.0)?;
        if vector.is_empty() {
            return invalid(
                &format!("{field}[{}]", embedder.0),
                "centroid vector must be non-empty",
            );
        }
        for (idx, value) in vector.iter().enumerate() {
            validate_finite(&format!("{field}[{}][{idx}]", embedder.0), *value)?;
        }
    }
    Ok(())
}

fn validate_embedder_scalar_map(
    field: &str,
    scalars: &BTreeMap<EmbedderId, f32>,
    centroid_by_embedder: &BTreeMap<EmbedderId, Vec<f32>>,
    kind: ScalarKind,
) -> Result<(), MejepaInferError> {
    if scalars.len() != centroid_by_embedder.len() {
        return Err(MejepaInferError::DimMismatch {
            expected: centroid_by_embedder.len(),
            actual: scalars.len(),
            context: field.to_string(),
        });
    }
    for embedder in centroid_by_embedder.keys() {
        let value = scalars
            .get(embedder)
            .ok_or_else(|| MejepaInferError::InvalidInput {
                field: field.to_string(),
                detail: format!("missing scalar for embedder {}", embedder.0),
            })?;
        validate_scalar(field, *value, kind)?;
    }
    Ok(())
}

fn validate_pair_metrics(
    field: &str,
    metrics: &[FingerprintPairMetric],
    centroid_by_embedder: &BTreeMap<EmbedderId, Vec<f32>>,
    kind: ScalarKind,
) -> Result<(), MejepaInferError> {
    let mut seen = BTreeSet::new();
    for metric in metrics {
        validate_text(&format!("{field}.left_embedder"), &metric.left_embedder.0)?;
        validate_text(&format!("{field}.right_embedder"), &metric.right_embedder.0)?;
        if metric.left_embedder >= metric.right_embedder {
            return invalid(
                field,
                "pair metrics must be stored once with left_embedder < right_embedder",
            );
        }
        if !centroid_by_embedder.contains_key(&metric.left_embedder)
            || !centroid_by_embedder.contains_key(&metric.right_embedder)
        {
            return invalid(
                field,
                "pair metric references an embedder absent from centroid_by_embedder",
            );
        }
        let pair = (metric.left_embedder.clone(), metric.right_embedder.clone());
        if !seen.insert(pair) {
            return invalid(field, "duplicate pair metric");
        }
        validate_scalar(field, metric.value, kind)?;
    }
    Ok(())
}

fn validate_embedder_id_list(
    field: &str,
    embedders: &[EmbedderId],
) -> Result<(), MejepaInferError> {
    if embedders.is_empty() {
        return invalid(field, "must contain at least one embedder");
    }
    let mut seen = BTreeSet::new();
    for embedder in embedders {
        validate_text(field, &embedder.0)?;
        if !seen.insert(embedder) {
            return invalid(field, "embedder list contains duplicates");
        }
    }
    Ok(())
}

fn validate_scalar(field: &str, value: f32, kind: ScalarKind) -> Result<(), MejepaInferError> {
    match kind {
        ScalarKind::Cosine => validate_cosine(field, value),
        ScalarKind::NonNegative => validate_nonnegative_finite(field, value),
    }
}

fn validate_text(field: &str, value: &str) -> Result<(), MejepaInferError> {
    if value.trim().is_empty() {
        return invalid(field, "must be non-empty");
    }
    if value.len() > MAX_TEXT_BYTES {
        return invalid(field, &format!("exceeds {MAX_TEXT_BYTES} bytes"));
    }
    if value.bytes().any(|b| b < 0x20 || b == 0x7f) {
        return invalid(field, "contains a control character");
    }
    Ok(())
}

fn validate_nonzero_sha(field: &str, sha: &[u8; 32]) -> Result<(), MejepaInferError> {
    if sha.iter().all(|b| *b == 0) {
        return invalid(field, "sha256 must be non-zero");
    }
    Ok(())
}

fn validate_optional_probability(field: &str, value: Option<f32>) -> Result<(), MejepaInferError> {
    if let Some(value) = value {
        validate_probability(field, value)?;
    }
    Ok(())
}

fn validate_probability(field: &str, value: f32) -> Result<(), MejepaInferError> {
    validate_finite(field, value)?;
    if !(0.0..=1.0).contains(&value) {
        return invalid(field, &format!("must be in [0, 1]; got {value}"));
    }
    Ok(())
}

fn validate_cosine(field: &str, value: f32) -> Result<(), MejepaInferError> {
    validate_finite(field, value)?;
    if !(-1.0..=1.0).contains(&value) {
        return invalid(field, &format!("must be in [-1, 1]; got {value}"));
    }
    Ok(())
}

fn validate_nonnegative_finite(field: &str, value: f32) -> Result<(), MejepaInferError> {
    validate_finite(field, value)?;
    if value < 0.0 {
        return invalid(field, &format!("must be non-negative; got {value}"));
    }
    Ok(())
}

fn validate_finite(field: &str, value: f32) -> Result<(), MejepaInferError> {
    if !value.is_finite() {
        return Err(MejepaInferError::NanDetected {
            nan_source: field.to_string(),
            detail: format!("{field} is {value}; must be finite"),
        });
    }
    Ok(())
}

fn invalid<T>(field: &str, detail: &str) -> Result<T, MejepaInferError> {
    Err(MejepaInferError::InvalidInput {
        field: field.to_string(),
        detail: detail.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_id_is_stable_for_same_kind_and_source() {
        let kind = FingerprintKind::KnownBad {
            repo: "django/django".to_string(),
            mutation_category: MutationCategory::OffByOne,
            failure_mode: FailureModeClass::OffByOne,
            exception_class: Some(ExceptionClass::AssertionError),
        };
        let left = FailureShapeFingerprint::canonical_id(&kind, "aiwonder-prod-20260515")
            .expect("left canonical id");
        let right = FailureShapeFingerprint::canonical_id(&kind, "aiwonder-prod-20260515")
            .expect("right canonical id");
        assert_eq!(left, right);
        assert_ne!(left, FingerprintId::default());
    }

    #[test]
    fn classifier_emits_pass_for_known_good_only() {
        let good = fixture_fingerprint(FingerprintKind::KnownGood {
            repo: Some("django__django".to_string()),
            gold_patch_count: 2,
        });
        let bad = fixture_fingerprint_with_centroid(
            FingerprintKind::KnownBad {
                repo: "django__django".to_string(),
                mutation_category: MutationCategory::OffByOne,
                failure_mode: FailureModeClass::OffByOne,
                exception_class: Some(ExceptionClass::AssertionError),
            },
            vec![0.0, 1.0],
        );
        let observation = vectors([("e1", vec![1.0, 0.0])]);

        let classification = classify_failure_fingerprint_observation(
            &[good, bad],
            &observation,
            FingerprintClassifierConfig::default(),
        )
        .expect("classify known good");

        assert_eq!(classification.verdict, Verdict::Pass);
        assert_eq!(
            classification.reason,
            FingerprintDecisionReason::KnownGoodOnly
        );
        assert_eq!(classification.matched_known_good_count, 1);
        assert_eq!(classification.matched_known_bad_count, 0);
    }

    #[test]
    fn classifier_emits_fail_for_known_bad_only() {
        let good = fixture_fingerprint(FingerprintKind::KnownGood {
            repo: Some("django__django".to_string()),
            gold_patch_count: 2,
        });
        let bad = fixture_fingerprint_with_centroid(
            FingerprintKind::KnownBad {
                repo: "django__django".to_string(),
                mutation_category: MutationCategory::OffByOne,
                failure_mode: FailureModeClass::OffByOne,
                exception_class: Some(ExceptionClass::AssertionError),
            },
            vec![0.0, 1.0],
        );
        let observation = vectors([("e1", vec![0.0, 1.0])]);

        let classification = classify_failure_fingerprint_observation(
            &[good, bad],
            &observation,
            FingerprintClassifierConfig::default(),
        )
        .expect("classify known bad");

        assert_eq!(classification.verdict, Verdict::Fail);
        assert_eq!(
            classification.reason,
            FingerprintDecisionReason::KnownBadOnly
        );
        assert_eq!(classification.matched_known_good_count, 0);
        assert_eq!(classification.matched_known_bad_count, 1);
    }

    #[test]
    fn classifier_emits_ood_when_no_fingerprint_matches() {
        let good = fixture_fingerprint(FingerprintKind::KnownGood {
            repo: Some("django__django".to_string()),
            gold_patch_count: 2,
        });
        let bad = fixture_fingerprint_with_centroid(
            FingerprintKind::KnownBad {
                repo: "django__django".to_string(),
                mutation_category: MutationCategory::OffByOne,
                failure_mode: FailureModeClass::OffByOne,
                exception_class: Some(ExceptionClass::AssertionError),
            },
            vec![0.0, 1.0],
        );
        let observation = vectors([("e1", vec![-1.0, 0.0])]);

        let classification = classify_failure_fingerprint_observation(
            &[good, bad],
            &observation,
            FingerprintClassifierConfig::default(),
        )
        .expect("classify ood");

        assert_eq!(classification.verdict, Verdict::OutOfDistribution);
        assert_eq!(
            classification.reason,
            FingerprintDecisionReason::NoKnownMatch
        );
        assert!(classification.primary_match.is_none());
    }

    #[test]
    fn classifier_abstains_on_known_good_known_bad_conflict() {
        let good = fixture_fingerprint(FingerprintKind::KnownGood {
            repo: Some("django__django".to_string()),
            gold_patch_count: 2,
        });
        let bad = fixture_fingerprint(FingerprintKind::KnownBad {
            repo: "django__django".to_string(),
            mutation_category: MutationCategory::OffByOne,
            failure_mode: FailureModeClass::OffByOne,
            exception_class: Some(ExceptionClass::AssertionError),
        });
        let observation = vectors([("e1", vec![1.0, 0.0])]);

        let classification = classify_failure_fingerprint_observation(
            &[good, bad],
            &observation,
            FingerprintClassifierConfig::default(),
        )
        .expect("classify conflict");

        assert_eq!(classification.verdict, Verdict::Abstain);
        assert_eq!(
            classification.reason,
            FingerprintDecisionReason::KnownGoodKnownBadConflict
        );
        assert_eq!(classification.matched_known_good_count, 1);
        assert_eq!(classification.matched_known_bad_count, 1);
    }

    #[test]
    fn classifier_fails_closed_on_missing_embedder_dimension_and_nonfinite() {
        let good = fixture_fingerprint(FingerprintKind::KnownGood {
            repo: Some("django__django".to_string()),
            gold_patch_count: 2,
        });
        let missing = BTreeMap::new();
        assert!(matches!(
            classify_failure_fingerprint_observation(
                std::slice::from_ref(&good),
                &missing,
                FingerprintClassifierConfig::default(),
            )
            .unwrap_err(),
            MejepaInferError::InvalidInput { .. }
        ));

        let dim_mismatch = vectors([("e1", vec![1.0, 0.0, 0.0])]);
        assert!(matches!(
            classify_failure_fingerprint_observation(
                std::slice::from_ref(&good),
                &dim_mismatch,
                FingerprintClassifierConfig::default(),
            )
            .unwrap_err(),
            MejepaInferError::DimMismatch { .. }
        ));

        let nonfinite = vectors([("e1", vec![f32::NAN, 0.0])]);
        assert!(matches!(
            classify_failure_fingerprint_observation(
                std::slice::from_ref(&good),
                &nonfinite,
                FingerprintClassifierConfig::default(),
            )
            .unwrap_err(),
            MejepaInferError::NanDetected { .. }
        ));
    }

    fn fixture_fingerprint(kind: FingerprintKind) -> FailureShapeFingerprint {
        fixture_fingerprint_with_centroid(kind, vec![1.0, 0.0])
    }

    fn fixture_fingerprint_with_centroid(
        kind: FingerprintKind,
        centroid: Vec<f32>,
    ) -> FailureShapeFingerprint {
        let source_corpus = "unit-test-corpus";
        let fingerprint_id =
            FailureShapeFingerprint::canonical_id(&kind, source_corpus).expect("canonical id");
        let embedder = EmbedderId("e1".to_string());
        let oracle_outcome = match kind {
            FingerprintKind::KnownGood { .. } => Some(OracleOutcome::Pass),
            FingerprintKind::KnownBad { .. } => Some(OracleOutcome::Fail),
            FingerprintKind::Unknown { .. } => None,
        };
        FailureShapeFingerprint {
            schema_version: FAILURE_FINGERPRINT_SCHEMA_VERSION,
            fingerprint_id,
            name: kind.class_slug(),
            kind,
            source_corpus: source_corpus.to_string(),
            source_manifest_sha256: Some([0x5a; 32]),
            centroid_by_embedder: BTreeMap::from([(embedder.clone(), centroid)]),
            variance_by_embedder: BTreeMap::from([(embedder.clone(), 0.01)]),
            tau_by_embedder: BTreeMap::from([(embedder, 0.90)]),
            pairwise_cosine: Vec::new(),
            pairwise_mutual_information: Vec::new(),
            reference_chunks: vec![ChunkId("chunk-1".to_string())],
            n_references: 1,
            oracle_outcome,
            is_canonical: true,
            frozen_at_unix_ms: 1,
            confidence: FingerprintConfidence {
                classification_accuracy: Some(1.0),
                classification_precision: Some(1.0),
                unknown_recall: None,
                calibration_observations: 2,
                calibration_state: FingerprintCalibrationState::Calibrated,
            },
        }
    }

    fn vectors<const N: usize>(items: [(&str, Vec<f32>); N]) -> BTreeMap<EmbedderId, Vec<f32>> {
        items
            .into_iter()
            .map(|(embedder, vector)| (EmbedderId(embedder.to_string()), vector))
            .collect()
    }

    // ----- TASK-FP-008 (#317) — Fisher / Dormancy snapshot validators -----

    fn sample_fingerprint_id() -> FingerprintId {
        let kind = FingerprintKind::KnownGood {
            repo: Some("repo/test".to_string()),
            gold_patch_count: 1,
        };
        FailureShapeFingerprint::canonical_id(&kind, "task-fp-008-test").unwrap()
    }

    fn theta_for(fisher_dims: &BTreeMap<u32, f32>) -> BTreeMap<u32, f32> {
        fisher_dims.keys().map(|d| (*d, 0.5_f32)).collect()
    }

    #[test]
    fn fisher_snapshot_validates_finite_nonneg_with_signal() {
        let fisher = BTreeMap::from([(0_u32, 0.1_f32), (1, 0.0), (2, 0.5)]);
        let snapshot = FingerprintFisherSnapshot {
            fingerprint_id: sample_fingerprint_id(),
            calibrated_at_unix_ms: 1_779_000_000_000,
            sample_count: 32,
            theta_star_by_dimension: theta_for(&fisher),
            fisher_by_dimension: fisher,
        };
        snapshot.validate().unwrap();
    }

    #[test]
    fn fisher_snapshot_rejects_all_zero() {
        let fisher = BTreeMap::from([(0_u32, 0.0_f32), (1, 0.0)]);
        let snapshot = FingerprintFisherSnapshot {
            fingerprint_id: sample_fingerprint_id(),
            calibrated_at_unix_ms: 1_779_000_000_000,
            sample_count: 32,
            theta_star_by_dimension: theta_for(&fisher),
            fisher_by_dimension: fisher,
        };
        let err = snapshot.validate().unwrap_err().to_string();
        assert!(
            err.contains("all dimensions zero"),
            "expected all-zero rejection, got: {err}"
        );
    }

    #[test]
    fn fisher_snapshot_rejects_non_finite() {
        let fisher = BTreeMap::from([(0_u32, f32::INFINITY)]);
        let snapshot = FingerprintFisherSnapshot {
            fingerprint_id: sample_fingerprint_id(),
            calibrated_at_unix_ms: 1_779_000_000_000,
            sample_count: 32,
            theta_star_by_dimension: theta_for(&fisher),
            fisher_by_dimension: fisher,
        };
        assert!(snapshot.validate().is_err());
    }

    #[test]
    fn fisher_snapshot_rejects_zero_sample_count() {
        let fisher = BTreeMap::from([(0_u32, 0.5_f32)]);
        let snapshot = FingerprintFisherSnapshot {
            fingerprint_id: sample_fingerprint_id(),
            calibrated_at_unix_ms: 1_779_000_000_000,
            sample_count: 0,
            theta_star_by_dimension: theta_for(&fisher),
            fisher_by_dimension: fisher,
        };
        assert!(snapshot.validate().is_err());
    }

    #[test]
    fn fisher_snapshot_rejects_negative_value() {
        let fisher = BTreeMap::from([(0_u32, -0.1_f32)]);
        let snapshot = FingerprintFisherSnapshot {
            fingerprint_id: sample_fingerprint_id(),
            calibrated_at_unix_ms: 1_779_000_000_000,
            sample_count: 32,
            theta_star_by_dimension: theta_for(&fisher),
            fisher_by_dimension: fisher,
        };
        assert!(snapshot.validate().is_err());
    }

    #[test]
    fn fisher_snapshot_rejects_theta_dimension_mismatch() {
        let fisher = BTreeMap::from([(0_u32, 0.1_f32), (1, 0.5)]);
        let snapshot = FingerprintFisherSnapshot {
            fingerprint_id: sample_fingerprint_id(),
            calibrated_at_unix_ms: 1_779_000_000_000,
            sample_count: 32,
            theta_star_by_dimension: BTreeMap::from([(0, 0.5_f32)]),
            fisher_by_dimension: fisher,
        };
        let err = snapshot.validate().unwrap_err().to_string();
        assert!(
            err.contains("dimensions") || err.contains("cover the same"),
            "expected theta-vs-fisher dimension-mismatch rejection, got: {err}"
        );
    }

    #[test]
    fn fisher_snapshot_round_trips_bincode() {
        let fisher = BTreeMap::from([(0_u32, 0.42_f32), (1, 0.0), (2, 1.1)]);
        let snapshot = FingerprintFisherSnapshot {
            fingerprint_id: sample_fingerprint_id(),
            calibrated_at_unix_ms: 1_779_000_000_000,
            sample_count: 16,
            theta_star_by_dimension: theta_for(&fisher),
            fisher_by_dimension: fisher,
        };
        let bytes = bincode::serialize(&snapshot).unwrap();
        let roundtripped: FingerprintFisherSnapshot = bincode::deserialize(&bytes).unwrap();
        assert_eq!(snapshot, roundtripped);
    }

    #[test]
    fn dormancy_snapshot_validates_bounded_finite() {
        let snapshot = FingerprintDormancySnapshot {
            fingerprint_id: sample_fingerprint_id(),
            recorded_at_unix_ms: 1_779_000_000_000,
            window_steps: 1000,
            dormancy_ema_by_dimension: BTreeMap::from([(0, 0.0), (1, 0.5), (2, 1.0)]),
            dormancy_reset_count: 3,
        };
        snapshot.validate().unwrap();
    }

    #[test]
    fn dormancy_snapshot_rejects_out_of_range() {
        let snapshot = FingerprintDormancySnapshot {
            fingerprint_id: sample_fingerprint_id(),
            recorded_at_unix_ms: 1_779_000_000_000,
            window_steps: 1000,
            dormancy_ema_by_dimension: BTreeMap::from([(0, 1.5)]),
            dormancy_reset_count: 0,
        };
        assert!(snapshot.validate().is_err());
    }

    #[test]
    fn dormancy_snapshot_rejects_nan_with_reset_guidance() {
        let snapshot = FingerprintDormancySnapshot {
            fingerprint_id: sample_fingerprint_id(),
            recorded_at_unix_ms: 1_779_000_000_000,
            window_steps: 1000,
            dormancy_ema_by_dimension: BTreeMap::from([(0, f32::NAN)]),
            dormancy_reset_count: 0,
        };
        let err = snapshot.validate().unwrap_err().to_string();
        assert!(
            err.contains("not finite"),
            "expected NaN-not-finite rejection with reset guidance, got: {err}"
        );
    }

    #[test]
    fn dormancy_snapshot_rejects_empty_dim_map() {
        let snapshot = FingerprintDormancySnapshot {
            fingerprint_id: sample_fingerprint_id(),
            recorded_at_unix_ms: 1_779_000_000_000,
            window_steps: 1000,
            dormancy_ema_by_dimension: BTreeMap::new(),
            dormancy_reset_count: 0,
        };
        assert!(snapshot.validate().is_err());
    }

    #[test]
    fn dormancy_snapshot_round_trips_bincode() {
        let snapshot = FingerprintDormancySnapshot {
            fingerprint_id: sample_fingerprint_id(),
            recorded_at_unix_ms: 1_779_000_000_000,
            window_steps: 1_000_000,
            dormancy_ema_by_dimension: BTreeMap::from([(0, 0.12), (1, 0.99)]),
            dormancy_reset_count: 7,
        };
        let bytes = bincode::serialize(&snapshot).unwrap();
        let roundtripped: FingerprintDormancySnapshot = bincode::deserialize(&bytes).unwrap();
        assert_eq!(snapshot, roundtripped);
    }
}
