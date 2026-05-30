use std::collections::{BTreeMap, BTreeSet};
use std::io::Cursor;
use std::path::{Component, Path, PathBuf};

use context_graph_mejepa_instruments::OracleVerdict;
use serde::{Deserialize, Serialize};

use crate::constellation_intelligence::ConstellationIntelligenceEvidence;
use crate::error::MejepaInferError;
use crate::failure_fingerprint::FingerprintCandidateScore;

const MAX_ID_BYTES: usize = 512;
const MAX_TESTS: usize = 1024;
const MAX_CHUNKS: usize = 4096;
/// REQ-FLYWHEEL-10 bounds. Empirically chosen to match the 21-active-embedder
/// ceiling, the 100k chunk fanout cap (`MAX_EXEMPLARS_PER_BUCKET`), and the
/// 64 KiB ledger payload limit. Numbers must stay loose enough to never reject
/// real production traffic, but tight enough to fail-closed on accidental DoS.
const MAX_EMBEDDERS_PER_PANEL: usize = 256;
const MAX_PAIRWISE_SIGNALS: usize = MAX_EMBEDDERS_PER_PANEL * MAX_EMBEDDERS_PER_PANEL;
const MAX_EXEMPLARS_PER_BUCKET: usize = 100_000;
const MAX_AGENT_FEEDBACK_PAYLOAD_BYTES: usize = 65_536;
const MAX_AGENT_EXPLANATION_BYTES: usize = 4_096;
const MAX_AGENT_ID_BYTES: usize = 256;
const MAX_ENTITIES: usize = 4096;
const MAX_SKILL_CITATIONS: usize = 512;
const MAX_PATCH_HUNKS: usize = 4096;
const MAX_HIERARCHY_LEVELS: usize = MAX_PATCH_HUNKS * 4;
const MAX_HIERARCHY_SCOPE_ID_BYTES: usize = 1024;
const MAX_SLOT_ATTRIBUTIONS: usize = 512;
const MAX_HUNK_TEXT_BYTES: usize = 1_048_576;
const MAX_WITNESS_SEGMENT_BYTES: usize = 73 * 8192;
const MAX_COMMIT_MESSAGE_BYTES: usize = 8192;
const MAX_PROBLEM_STATEMENT_BYTES: usize = 65_536;
const MAX_ACCEPTED_LABEL_IDS: usize = 256;
const MAX_FAILURE_EVIDENCE_SET_IDS: usize = 128;
const MAX_ACTIVE_SKILL_IDS: usize = 128;
const MAX_ACTIVE_HIGHER_ABILITY_IDS: usize = 128;
const MAX_SOURCE_MEMBERSHIP_KEYS: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Language {
    Rust,
    Python,
    Javascript,
    Typescript,
    Go,
    Java,
    C,
    Cpp,
    CSharp,
    Ruby,
    Php,
}

impl Language {
    pub fn phase0_supported(self) -> bool {
        matches!(self, Self::Python)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OracleOutcome {
    Pass,
    Fail,
    OutOfDistribution,
    Abstain,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default)]
#[serde(transparent)]
pub struct TaskId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default)]
#[serde(transparent)]
pub struct TestId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default)]
#[serde(transparent)]
pub struct SkillId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default)]
#[serde(transparent)]
pub struct EmbedderId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default)]
#[serde(transparent)]
pub struct ChunkId(pub String);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConformalSet {
    pub outcomes: Vec<OracleOutcome>,
    pub alpha: f32,
    pub tau: f32,
    pub entropy_bits: f32,
}

impl ConformalSet {
    pub fn try_new(
        outcomes: Vec<OracleOutcome>,
        alpha: f32,
        tau: f32,
    ) -> Result<Self, MejepaInferError> {
        if outcomes.is_empty() {
            return Err(MejepaInferError::ConformalInsufficientSamples {
                language: None,
                expected: 1,
                actual: 0,
            });
        }
        if !alpha.is_finite() || alpha <= 0.0 || alpha >= 1.0 {
            return Err(MejepaInferError::InvalidInput {
                field: "alpha".to_string(),
                detail: format!("alpha must be finite and in (0, 1); got {alpha}"),
            });
        }
        if !tau.is_finite() || !(0.0..=1.0).contains(&tau) {
            return Err(MejepaInferError::InvalidInput {
                field: "tau".to_string(),
                detail: format!("tau must be finite and in [0, 1]; got {tau}"),
            });
        }
        let unique = outcomes.iter().copied().collect::<BTreeSet<_>>();
        if unique.len() != outcomes.len() {
            return Err(MejepaInferError::InvalidInput {
                field: "outcomes".to_string(),
                detail: "outcomes must be distinct".to_string(),
            });
        }
        let entropy_bits = if outcomes.len() <= 1 {
            0.0
        } else {
            (outcomes.len() as f32).log2()
        };
        Ok(Self {
            outcomes,
            alpha,
            tau,
            entropy_bits,
        })
    }

    pub fn validate(&self) -> Result<(), MejepaInferError> {
        if self.outcomes.is_empty() {
            return Err(MejepaInferError::ConformalInsufficientSamples {
                language: None,
                expected: 1,
                actual: 0,
            });
        }
        if !self.alpha.is_finite() || self.alpha <= 0.0 || self.alpha >= 1.0 {
            return Err(MejepaInferError::InvalidInput {
                field: "outcome_set.alpha".to_string(),
                detail: format!("alpha must be finite and in (0, 1); got {}", self.alpha),
            });
        }
        if !self.tau.is_finite() || !(0.0..=1.0).contains(&self.tau) {
            return Err(MejepaInferError::InvalidInput {
                field: "outcome_set.tau".to_string(),
                detail: format!("tau must be finite and in [0, 1]; got {}", self.tau),
            });
        }
        if !self.entropy_bits.is_finite() || self.entropy_bits < 0.0 {
            return Err(MejepaInferError::NanDetected {
                nan_source: "outcome_set.entropy_bits".to_string(),
                detail: format!(
                    "entropy_bits must be finite and non-negative; got {}",
                    self.entropy_bits
                ),
            });
        }
        let unique = self.outcomes.iter().copied().collect::<BTreeSet<_>>();
        if unique.len() != self.outcomes.len() {
            return Err(MejepaInferError::InvalidInput {
                field: "outcome_set.outcomes".to_string(),
                detail: "outcomes must be distinct".to_string(),
            });
        }
        let expected_entropy = if self.outcomes.len() <= 1 {
            0.0
        } else {
            (self.outcomes.len() as f32).log2()
        };
        if (self.entropy_bits - expected_entropy).abs() > f32::EPSILON {
            return Err(MejepaInferError::InvalidInput {
                field: "outcome_set.entropy_bits".to_string(),
                detail: format!(
                    "entropy_bits {} does not match outcome count {}",
                    self.entropy_bits,
                    self.outcomes.len()
                ),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HeadId {
    Panel,
    Oracle,
    FailureMode,
    EdgeCase,
    TechDebt,
    Perf,
    Security,
    Accuracy,
    Cost,
    Reasoning,
}

impl HeadId {
    pub const ALL: [Self; 10] = [
        Self::Panel,
        Self::Oracle,
        Self::FailureMode,
        Self::EdgeCase,
        Self::TechDebt,
        Self::Perf,
        Self::Security,
        Self::Accuracy,
        Self::Cost,
        Self::Reasoning,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Panel => "panel",
            Self::Oracle => "oracle",
            Self::FailureMode => "failure_mode",
            Self::EdgeCase => "edge_case",
            Self::TechDebt => "tech_debt",
            Self::Perf => "perf",
            Self::Security => "security",
            Self::Accuracy => "accuracy",
            Self::Cost => "cost",
            Self::Reasoning => "reasoning",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Pass,
    Fail,
    OutOfDistribution,
    Abstain,
    GuardRejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Info,
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConformalMethod {
    SplitConformal,
    Mondrian,
    Group,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConformalInterval {
    pub lower: f32,
    pub upper: f32,
    pub method: ConformalMethod,
    pub coverage_target: f32,
    pub empirical_coverage: f32,
}

impl Default for ConformalInterval {
    fn default() -> Self {
        Self {
            lower: 0.0,
            upper: 1.0,
            method: ConformalMethod::SplitConformal,
            coverage_target: 0.90,
            empirical_coverage: 0.0,
        }
    }
}

impl ConformalInterval {
    pub fn validate(&self, field: &str) -> Result<(), MejepaInferError> {
        validate_probability(&format!("{field}.lower"), self.lower)?;
        validate_probability(&format!("{field}.upper"), self.upper)?;
        validate_probability(&format!("{field}.coverage_target"), self.coverage_target)?;
        validate_probability(
            &format!("{field}.empirical_coverage"),
            self.empirical_coverage,
        )?;
        if self.lower > self.upper {
            return Err(MejepaInferError::InvalidInput {
                field: field.to_string(),
                detail: format!("lower {} must be <= upper {}", self.lower, self.upper),
            });
        }
        Ok(())
    }

    pub fn width(&self) -> f32 {
        self.upper - self.lower
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RootCauseClass {
    LogicError,
    StateError,
    ResourceError,
    ConcurrencyError,
    InterfaceError,
    EnvironmentError,
    ConfigurationError,
    TestQualityError,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TestOutcome {
    Pass,
    Fail,
    Error,
    Skip,
    Flaky,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TestDeltaKind {
    PassToFail,
    FailToPass,
    NewFailure,
    NewPass,
    NowSkipped,
    NowFlaky,
    Unchanged,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PredictedFailureMode {
    pub failure_class: FailureModeClass,
    pub chunk: ChunkId,
    pub line_range: (u32, u32),
    pub confidence: f32,
    pub severity: Severity,
    pub explanation: String,
    pub contributing_embedders: Vec<EmbedderId>,
    pub root_cause_class: RootCauseClass,
}

impl PredictedFailureMode {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        self.chunk.validate("predicted_failure_mode.chunk")?;
        validate_line_range("predicted_failure_mode.line_range", self.line_range)?;
        validate_probability("predicted_failure_mode.confidence", self.confidence)?;
        validate_bounded_text(
            "predicted_failure_mode.explanation",
            &self.explanation,
            MAX_COMMIT_MESSAGE_BYTES,
        )?;
        if self.contributing_embedders.len() > MAX_EMBEDDERS_PER_PANEL {
            return Err(MejepaInferError::DimMismatch {
                expected: MAX_EMBEDDERS_PER_PANEL,
                actual: self.contributing_embedders.len(),
                context: "predicted_failure_mode.contributing_embedders exceeds maximum"
                    .to_string(),
            });
        }
        for (idx, embedder) in self.contributing_embedders.iter().enumerate() {
            embedder.validate(&format!(
                "predicted_failure_mode.contributing_embedders[{idx}]"
            ))?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PredictedTestOutcome {
    pub test_id: TestId,
    pub current_outcome: TestOutcome,
    pub predicted_outcome: TestOutcome,
    pub delta_kind: TestDeltaKind,
    pub confidence: f32,
    pub why: PredictedFailureMode,
}

impl PredictedTestOutcome {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        self.test_id.validate("predicted_test_outcome.test_id")?;
        validate_probability("predicted_test_outcome.confidence", self.confidence)?;
        self.why.validate()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PredictedWorks {
    pub chunk: ChunkId,
    pub line_range: (u32, u32),
    pub claim: String,
    pub confidence: f32,
    pub supporting_embedders: Vec<EmbedderId>,
    pub similar_known_good_exemplars: Vec<ExemplarMatch>,
    pub evidence_strength: f32,
}

impl PredictedWorks {
    pub fn is_high_confidence(&self) -> bool {
        self.confidence >= 0.8 && self.evidence_strength >= 0.7
    }

    pub fn validate(&self) -> Result<(), MejepaInferError> {
        self.chunk.validate("predicted_works.chunk")?;
        validate_line_range("predicted_works.line_range", self.line_range)?;
        validate_bounded_text(
            "predicted_works.claim",
            &self.claim,
            MAX_COMMIT_MESSAGE_BYTES,
        )?;
        validate_probability("predicted_works.confidence", self.confidence)?;
        validate_probability("predicted_works.evidence_strength", self.evidence_strength)?;
        if self.supporting_embedders.is_empty() {
            return Err(MejepaInferError::InvalidInput {
                field: "predicted_works.supporting_embedders".to_string(),
                detail: "supporting_embedders must be non-empty".to_string(),
            });
        }
        if self.supporting_embedders.len() > MAX_EMBEDDERS_PER_PANEL {
            return Err(MejepaInferError::DimMismatch {
                expected: MAX_EMBEDDERS_PER_PANEL,
                actual: self.supporting_embedders.len(),
                context: "predicted_works.supporting_embedders exceeds maximum".to_string(),
            });
        }
        for (idx, embedder) in self.supporting_embedders.iter().enumerate() {
            embedder.validate(&format!("predicted_works.supporting_embedders[{idx}]"))?;
        }
        if self.similar_known_good_exemplars.len() > MAX_EXEMPLARS_PER_BUCKET {
            return Err(MejepaInferError::DimMismatch {
                expected: MAX_EXEMPLARS_PER_BUCKET,
                actual: self.similar_known_good_exemplars.len(),
                context: "predicted_works.similar_known_good_exemplars exceeds maximum".to_string(),
            });
        }
        validate_items(
            "predicted_works.similar_known_good_exemplars",
            &self.similar_known_good_exemplars,
            ExemplarMatch::validate,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UncoveredPath {
    pub chunk: ChunkId,
    pub line_range: (u32, u32),
    pub path_description: String,
    pub defect_probability: f32,
    pub confidence: f32,
    pub evidence: String,
}

impl UncoveredPath {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        self.chunk.validate("uncovered_path.chunk")?;
        validate_line_range("uncovered_path.line_range", self.line_range)?;
        validate_bounded_text(
            "uncovered_path.path_description",
            &self.path_description,
            MAX_COMMIT_MESSAGE_BYTES,
        )?;
        validate_probability("uncovered_path.defect_probability", self.defect_probability)?;
        validate_probability("uncovered_path.confidence", self.confidence)?;
        validate_bounded_text(
            "uncovered_path.evidence",
            &self.evidence,
            MAX_COMMIT_MESSAGE_BYTES,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FlakyTestCandidate {
    pub test_id: TestId,
    pub serial_pass_probability: f32,
    pub parallel_pass_probability: f32,
    pub confidence: f32,
    pub evidence: String,
}

impl FlakyTestCandidate {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        self.test_id.validate("flaky_test_candidate.test_id")?;
        validate_probability(
            "flaky_test_candidate.serial_pass_probability",
            self.serial_pass_probability,
        )?;
        validate_probability(
            "flaky_test_candidate.parallel_pass_probability",
            self.parallel_pass_probability,
        )?;
        validate_probability("flaky_test_candidate.confidence", self.confidence)?;
        validate_bounded_text(
            "flaky_test_candidate.evidence",
            &self.evidence,
            MAX_COMMIT_MESSAGE_BYTES,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GuardViolation {
    pub embedder: EmbedderId,
    pub chunk: ChunkId,
    pub centroid_id: String,
    pub cosine: f32,
    pub threshold_tau_m: f32,
    pub deficit: f32,
}

impl GuardViolation {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        self.embedder.validate("guard_violation.embedder")?;
        self.chunk.validate("guard_violation.chunk")?;
        validate_non_empty_id("guard_violation.centroid_id", &self.centroid_id)?;
        validate_cosine("guard_violation.cosine", self.cosine)?;
        validate_probability("guard_violation.threshold_tau_m", self.threshold_tau_m)?;
        if !self.deficit.is_finite() || self.deficit < 0.0 {
            return Err(MejepaInferError::NanDetected {
                nan_source: "guard_violation.deficit".to_string(),
                detail: format!(
                    "deficit must be finite and non-negative; got {}",
                    self.deficit
                ),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExemplarMatch {
    /// TASK-PY-G-042 (#261) — stable id of the exemplar prediction row.
    /// `serde(default)` preserves deserialization of pre-index live rows.
    #[serde(default)]
    pub exemplar_prediction_id: Option<[u8; 16]>,
    pub task_id: TaskId,
    pub mutation_kind: crate::eval::MutationCategory,
    pub similarity_score: f32,
    pub diff_summary: String,
    pub oracle_outcome: OracleVerdict,
    #[serde(default)]
    pub failure_mode_class: Option<FailureModeClass>,
    #[serde(default)]
    pub evidence_path: Option<String>,
    pub witness_hash: WitnessHash,
}

/// TASK-FP-010 (#319) — operator/agent-facing evidence that a live
/// `RealityPrediction` matched a row in the failure-shape fingerprint
/// catalog. Always paired with one of the five `Verdict` outcomes.
///
/// * `Pass` / `Fail` verdicts: `matched_fingerprint = Some(highest-cosine
///   matched candidate)` and `unknown_candidate_id = None`.
/// * `Abstain` (KnownGood/KnownBad conflict): `matched_fingerprint = Some(
///   highest-ranked candidate regardless of kind)` and
///   `unknown_candidate_id = None`.
/// * `OutOfDistribution`: `matched_fingerprint = None` and
///   `unknown_candidate_id = Some(candidate id enqueued for active learning)`.
/// * `GuardRejected`: both fields `None`.
///
/// The struct is intentionally small — the full `FingerprintClassification`
/// (with `ranked_matches` and per-embedder scores) is fetched separately via
/// `mejepa_explain_prediction`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MatchedFingerprintEvidence {
    pub fingerprint_id: [u8; 32],
    pub name: String,
    pub kind_slug: String,
    pub oracle_outcome: Option<OracleOutcome>,
    pub mean_cosine: f32,
    pub min_margin: f32,
    pub n_references: usize,
    pub calibration_state: String,
}

impl MatchedFingerprintEvidence {
    pub fn from_candidate(candidate: &FingerprintCandidateScore) -> Result<Self, MejepaInferError> {
        let value = Self {
            fingerprint_id: candidate.fingerprint_id.0,
            name: candidate.name.clone(),
            kind_slug: candidate.kind.kind_slug().to_string(),
            oracle_outcome: candidate.oracle_outcome,
            mean_cosine: candidate.mean_cosine,
            min_margin: candidate.min_margin,
            n_references: candidate.n_references,
            calibration_state: candidate.calibration_state.slug().to_string(),
        };
        value.validate()?;
        Ok(value)
    }

    pub fn validate(&self) -> Result<(), MejepaInferError> {
        if self.fingerprint_id.iter().all(|byte| *byte == 0) {
            return Err(MejepaInferError::InvalidInput {
                field: "matched_fingerprint.fingerprint_id".to_string(),
                detail: "fingerprint_id must be non-zero".to_string(),
            });
        }
        validate_bounded_text(
            "matched_fingerprint.name",
            &self.name,
            MAX_COMMIT_MESSAGE_BYTES,
        )?;
        validate_bounded_text("matched_fingerprint.kind_slug", &self.kind_slug, 64)?;
        validate_bounded_text(
            "matched_fingerprint.calibration_state",
            &self.calibration_state,
            64,
        )?;
        if !self.mean_cosine.is_finite() {
            return Err(MejepaInferError::InvalidInput {
                field: "matched_fingerprint.mean_cosine".to_string(),
                detail: "mean_cosine must be finite".to_string(),
            });
        }
        if !self.min_margin.is_finite() {
            return Err(MejepaInferError::InvalidInput {
                field: "matched_fingerprint.min_margin".to_string(),
                detail: "min_margin must be finite".to_string(),
            });
        }
        if self.n_references == 0 {
            return Err(MejepaInferError::InvalidInput {
                field: "matched_fingerprint.n_references".to_string(),
                detail: "n_references must be >= 1".to_string(),
            });
        }
        Ok(())
    }
}

impl ExemplarMatch {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        self.task_id.validate("exemplar_match.task_id")?;
        validate_probability("exemplar_match.similarity_score", self.similarity_score)?;
        validate_bounded_text(
            "exemplar_match.diff_summary",
            &self.diff_summary,
            MAX_COMMIT_MESSAGE_BYTES,
        )?;
        if let Some(prediction_id) = self.exemplar_prediction_id {
            if prediction_id.iter().all(|byte| *byte == 0) {
                return Err(MejepaInferError::InvalidInput {
                    field: "exemplar_match.exemplar_prediction_id".to_string(),
                    detail: "exemplar_prediction_id must be non-zero when present".to_string(),
                });
            }
        }
        if let Some(path) = &self.evidence_path {
            validate_bounded_text(
                "exemplar_match.evidence_path",
                path,
                MAX_COMMIT_MESSAGE_BYTES,
            )?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeCaseClass {
    EmptyInput,
    SingleElement,
    BoundaryValue,
    UnicodeEdge,
    WhitespaceOnly,
    LeadingTrailingWhitespace,
    DuplicateKeys,
    CircularReference,
    SelfReference,
    DeepNesting,
    LargeInput,
    UninitializedField,
    OptionalNotPresent,
    DefaultValueLeakage,
    ConcurrentAccess,
    PartialFailure,
    CrossPlatform,
    TimezoneTransition,
    Locale,
    NetworkPartition,
    DiskFull,
    PermissionDenied,
    Other,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PredictedEdgeCase {
    pub edge_class: EdgeCaseClass,
    pub chunk: ChunkId,
    pub line_range: (u32, u32),
    pub triggering_input_description: String,
    pub covered_by_test: bool,
    pub confidence: f32,
}

impl PredictedEdgeCase {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        self.chunk.validate("predicted_edge_case.chunk")?;
        validate_line_range("predicted_edge_case.line_range", self.line_range)?;
        validate_bounded_text(
            "predicted_edge_case.triggering_input_description",
            &self.triggering_input_description,
            MAX_COMMIT_MESSAGE_BYTES,
        )?;
        validate_probability("predicted_edge_case.confidence", self.confidence)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LatentBugClass {
    ShadowedVariable,
    ShadowedImport,
    InconsistentNullCheck,
    InconsistentErrorHandling,
    UnreachableBranch,
    DuplicateCondition,
    SwitchFallthrough,
    LooseEquality,
    BareException,
    LoggerNotConfigured,
    ForgottenAwait,
    ForgottenYield,
    ForgottenReturn,
    ResourceNotReleasedOnError,
    LockHeldAcrossAwait,
    NondeterministicIteration,
    HashableMutation,
    Other,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PredictedLatentBug {
    pub bug_class: LatentBugClass,
    pub chunk: ChunkId,
    pub line_range: (u32, u32),
    pub confidence: f32,
    pub severity: Severity,
    pub explanation: String,
}

impl PredictedLatentBug {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        self.chunk.validate("predicted_latent_bug.chunk")?;
        validate_line_range("predicted_latent_bug.line_range", self.line_range)?;
        validate_probability("predicted_latent_bug.confidence", self.confidence)?;
        validate_bounded_text(
            "predicted_latent_bug.explanation",
            &self.explanation,
            MAX_COMMIT_MESSAGE_BYTES,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TechDebtClass {
    HighCyclomaticComplexity,
    DeepNesting,
    LongFunction,
    LongParameterList,
    LongFile,
    DuplicatedCode,
    DataClumps,
    FeatureEnvy,
    ShotgunSurgery,
    GodObject,
    PrematureAbstraction,
    DeadFlagBranch,
    TodoWithoutOwner,
    CommentedOutCode,
    MagicNumber,
    StringLiteralForEnum,
    CopyOnWriteAbuse,
    SerdeUntagged,
    UnboundedRetry,
    UnboundedCache,
    Other,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PredictedTechDebt {
    pub debt_class: TechDebtClass,
    pub chunk: ChunkId,
    pub line_range: (u32, u32),
    pub severity: Severity,
    pub explanation: String,
}

impl PredictedTechDebt {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        self.chunk.validate("predicted_tech_debt.chunk")?;
        validate_line_range("predicted_tech_debt.line_range", self.line_range)?;
        validate_bounded_text(
            "predicted_tech_debt.explanation",
            &self.explanation,
            MAX_COMMIT_MESSAGE_BYTES,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeadCodeKind {
    UnusedFunction,
    UnusedClass,
    UnusedImport,
    UnusedVariable,
    UnreachableAfterReturn,
    UnreachableInfiniteLoop,
    DeadParameter,
    DeadField,
    AlwaysFalseBranch,
    AlwaysTrueBranch,
    Other,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PredictedDeadCode {
    pub kind: DeadCodeKind,
    pub chunk: ChunkId,
    pub line_range: (u32, u32),
    pub reason: String,
}

impl PredictedDeadCode {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        self.chunk.validate("predicted_dead_code.chunk")?;
        validate_line_range("predicted_dead_code.line_range", self.line_range)?;
        validate_bounded_text(
            "predicted_dead_code.reason",
            &self.reason,
            MAX_COMMIT_MESSAGE_BYTES,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RedundancyKind {
    DuplicatedFunction,
    DuplicatedTest,
    DuplicatedConstant,
    OverlappingErrorMessage,
    ParallelImplementation,
    ReinventedWheel,
    Other,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PredictedRedundancy {
    pub kind: RedundancyKind,
    pub chunk: ChunkId,
    pub also_at: Vec<ChunkId>,
    pub similarity: f32,
    pub explanation: String,
}

impl PredictedRedundancy {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        self.chunk.validate("predicted_redundancy.chunk")?;
        if self.also_at.len() > MAX_CHUNKS {
            return Err(MejepaInferError::DimMismatch {
                expected: MAX_CHUNKS,
                actual: self.also_at.len(),
                context: "predicted_redundancy.also_at exceeds maximum".to_string(),
            });
        }
        for (idx, chunk) in self.also_at.iter().enumerate() {
            chunk.validate(&format!("predicted_redundancy.also_at[{idx}]"))?;
        }
        validate_probability("predicted_redundancy.similarity", self.similarity)?;
        validate_bounded_text(
            "predicted_redundancy.explanation",
            &self.explanation,
            MAX_COMMIT_MESSAGE_BYTES,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PerfAxis {
    CpuMs,
    WallclockMs,
    RssKb,
    HeapAllocs,
    HeapBytes,
    StackBytes,
    BinarySize,
    StartupMs,
    P50LatencyMs,
    P99LatencyMs,
    Throughput,
    NetworkBytesIn,
    NetworkBytesOut,
    DiskReadBytes,
    DiskWriteBytes,
    GpuMemoryMb,
    Other,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PredictedPerfRegression {
    pub axis: PerfAxis,
    pub chunk: ChunkId,
    pub predicted_delta_pct: f32,
    pub baseline_value: Option<f64>,
    pub confidence: f32,
    pub explanation: String,
}

impl PredictedPerfRegression {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        self.chunk.validate("predicted_perf_regression.chunk")?;
        validate_finite_f32(
            "predicted_perf_regression.predicted_delta_pct",
            self.predicted_delta_pct,
        )?;
        validate_optional_f64(
            "predicted_perf_regression.baseline_value",
            self.baseline_value,
        )?;
        validate_probability("predicted_perf_regression.confidence", self.confidence)?;
        validate_bounded_text(
            "predicted_perf_regression.explanation",
            &self.explanation,
            MAX_COMMIT_MESSAGE_BYTES,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecurityConcernClass {
    SqlInjection,
    CommandInjection,
    PathTraversal,
    Xss,
    Csrf,
    Ssrf,
    XmlExternalEntity,
    Deserialization,
    InsecureRandom,
    HardcodedSecret,
    LoggingSecret,
    OverbroadException,
    InsecureCryptoAlgo,
    InsufficientCryptoKeyLength,
    MissingAuth,
    BrokenAccessControl,
    OpenRedirect,
    ClickjackingMissing,
    MissingTlsVerify,
    Other,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PredictedSecurityConcern {
    pub class: SecurityConcernClass,
    pub chunk: ChunkId,
    pub line_range: (u32, u32),
    pub cvss_estimate: Option<f32>,
    pub explanation: String,
}

impl PredictedSecurityConcern {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        self.chunk.validate("predicted_security_concern.chunk")?;
        validate_line_range("predicted_security_concern.line_range", self.line_range)?;
        if let Some(cvss) = self.cvss_estimate {
            if !cvss.is_finite() || !(0.0..=10.0).contains(&cvss) {
                return Err(MejepaInferError::NanDetected {
                    nan_source: "predicted_security_concern.cvss_estimate".to_string(),
                    detail: format!("cvss estimate must be finite in [0, 10]; got {cvss}"),
                });
            }
        }
        validate_bounded_text(
            "predicted_security_concern.explanation",
            &self.explanation,
            MAX_COMMIT_MESSAGE_BYTES,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccuracyMetric {
    F1,
    Precision,
    Recall,
    Auc,
    Accuracy,
    MeanAbsoluteError,
    MeanSquaredError,
    R2,
    PerplexitySnapshot,
    CalibrationError,
    BrierScore,
    EmbeddingSimilarity,
    DownstreamTaskScore(String),
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PredictedAccuracyDegradation {
    pub metric: AccuracyMetric,
    pub chunk: ChunkId,
    pub predicted_delta: f32,
    pub baseline_value: Option<f64>,
    pub confidence: f32,
    pub explanation: String,
}

impl PredictedAccuracyDegradation {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        self.chunk
            .validate("predicted_accuracy_degradation.chunk")?;
        validate_finite_f32(
            "predicted_accuracy_degradation.predicted_delta",
            self.predicted_delta,
        )?;
        validate_optional_f64(
            "predicted_accuracy_degradation.baseline_value",
            self.baseline_value,
        )?;
        validate_probability("predicted_accuracy_degradation.confidence", self.confidence)?;
        validate_bounded_text(
            "predicted_accuracy_degradation.explanation",
            &self.explanation,
            MAX_COMMIT_MESSAGE_BYTES,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CostAxis {
    CiMinutes,
    CiCpuSeconds,
    CiWallSeconds,
    EstimatedDollarsPerRun,
    LlmTokensPerCall,
    LlmDollarsPerCall,
    StorageBytes,
    EgressBytes,
    DependencyCount,
    BuildSeconds,
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PredictedCostRegression {
    pub axis: CostAxis,
    pub chunk: ChunkId,
    pub predicted_delta: f64,
    pub baseline_value: Option<f64>,
    pub explanation: String,
}

impl PredictedCostRegression {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        self.chunk.validate("predicted_cost_regression.chunk")?;
        validate_finite_f64(
            "predicted_cost_regression.predicted_delta",
            self.predicted_delta,
        )?;
        validate_optional_f64(
            "predicted_cost_regression.baseline_value",
            self.baseline_value,
        )?;
        validate_bounded_text(
            "predicted_cost_regression.explanation",
            &self.explanation,
            MAX_COMMIT_MESSAGE_BYTES,
        )
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningClass {
    Unknown,
    None,
    CodeOnly,
    Unsupported,
    Correct,
    MostlyCorrect,
    PlausibleButWrong,
    Hallucination,
    Hedging,
    Overclaiming,
    UnderClaiming,
    Calibrated,
    Apologetic,
    ConfidentCorrect,
    ConfidentWrong,
    #[default]
    Mute,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct AgentClaimGraph {
    pub raw_response: String,
    pub claims: Vec<AgentClaim>,
}

impl AgentClaimGraph {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        validate_bounded_text(
            "agent_claim_graph.raw_response",
            &self.raw_response,
            MAX_PROBLEM_STATEMENT_BYTES,
        )?;
        if self.claims.len() > MAX_ENTITIES {
            return Err(MejepaInferError::DimMismatch {
                expected: MAX_ENTITIES,
                actual: self.claims.len(),
                context: "agent_claim_graph.claims exceeds maximum".to_string(),
            });
        }
        validate_items(
            "agent_claim_graph.claims",
            &self.claims,
            AgentClaim::validate,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentClaim {
    pub kind: ClaimKind,
    pub text: String,
    pub references: Vec<ClaimReference>,
}

impl AgentClaim {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        validate_bounded_text("agent_claim.text", &self.text, MAX_COMMIT_MESSAGE_BYTES)?;
        if self.references.len() > MAX_ENTITIES {
            return Err(MejepaInferError::DimMismatch {
                expected: MAX_ENTITIES,
                actual: self.references.len(),
                context: "agent_claim.references exceeds maximum".to_string(),
            });
        }
        validate_items(
            "agent_claim.references",
            &self.references,
            ClaimReference::validate,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "value")]
pub enum ClaimKind {
    Added(SymbolRef),
    Modified(SymbolRef),
    Removed(SymbolRef),
    Renamed(SymbolRef, String),
    Tested(TestId),
    Verified(String),
    Refactored(String),
    Documented(SymbolRef),
    Other(String),
    Fixed(SymbolRef),
    WillPass(TestId),
    WillFail(TestId),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SymbolRef {
    pub symbol: String,
    pub file: Option<PathBuf>,
    pub line: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimReference {
    pub file: PathBuf,
    pub symbol: Option<String>,
    pub line: Option<u32>,
}

impl ClaimReference {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        validate_relative_path("claim_reference.file", &self.file)?;
        if let Some(symbol) = &self.symbol {
            validate_non_empty_id("claim_reference.symbol", symbol)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimReconciliation {
    pub claim: AgentClaim,
    pub status: ReconciliationStatus,
    pub evidence: Vec<EvidenceRow>,
}

impl ClaimReconciliation {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        self.claim.validate()?;
        if self.evidence.len() > MAX_ENTITIES {
            return Err(MejepaInferError::DimMismatch {
                expected: MAX_ENTITIES,
                actual: self.evidence.len(),
                context: "claim_reconciliation.evidence exceeds maximum".to_string(),
            });
        }
        validate_items(
            "claim_reconciliation.evidence",
            &self.evidence,
            EvidenceRow::validate,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReconciliationStatus {
    Confirmed,
    Missing,
    Modified,
    UnexpectedSideEffect,
    SuperficialMatch,
    AmbiguousRef,
    Unverifiable,
    Matched,
    ModifiedUnexpectedly,
    Ambiguous,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceRow {
    pub file: PathBuf,
    pub before_sha: [u8; 32],
    pub after_sha: [u8; 32],
    pub line_range: (u32, u32),
    pub contributing_shift_id: String,
}

impl EvidenceRow {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        validate_relative_path("evidence_row.file", &self.file)?;
        validate_line_range("evidence_row.line_range", self.line_range)?;
        validate_non_empty_id(
            "evidence_row.contributing_shift_id",
            &self.contributing_shift_id,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RealityImpact {
    pub observed_shifts: Vec<ShiftEntry>,
    pub predicted_files_changed: Vec<PathBuf>,
    pub observed_files_changed: Vec<PathBuf>,
    pub unexpected_files_changed: Vec<PathBuf>,
    pub predicted_test_outcomes: Vec<PredictedTestOutcome>,
    pub observed_test_outcomes: Vec<ObservedTestOutcome>,
    pub prediction_correctness: PredictionCorrectness,
}

impl RealityImpact {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        if self.observed_shifts.len() > MAX_PATCH_HUNKS {
            return Err(MejepaInferError::DimMismatch {
                expected: MAX_PATCH_HUNKS,
                actual: self.observed_shifts.len(),
                context: "reality_impact.observed_shifts exceeds maximum".to_string(),
            });
        }
        validate_items(
            "reality_impact.observed_shifts",
            &self.observed_shifts,
            ShiftEntry::validate,
        )?;
        validate_paths(
            "reality_impact.predicted_files_changed",
            &self.predicted_files_changed,
        )?;
        validate_paths(
            "reality_impact.observed_files_changed",
            &self.observed_files_changed,
        )?;
        validate_paths(
            "reality_impact.unexpected_files_changed",
            &self.unexpected_files_changed,
        )?;
        validate_items(
            "reality_impact.predicted_test_outcomes",
            &self.predicted_test_outcomes,
            PredictedTestOutcome::validate,
        )?;
        validate_items(
            "reality_impact.observed_test_outcomes",
            &self.observed_test_outcomes,
            ObservedTestOutcome::validate,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShiftEntry {
    pub shift_id: String,
    pub file: PathBuf,
    pub before_sha: [u8; 32],
    pub after_sha: [u8; 32],
}

impl ShiftEntry {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        validate_non_empty_id("shift_entry.shift_id", &self.shift_id)?;
        validate_relative_path("shift_entry.file", &self.file)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservedTestOutcome {
    pub test_id: TestId,
    pub outcome: TestOutcome,
    pub duration_ms: Option<u64>,
}

impl ObservedTestOutcome {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        self.test_id.validate("observed_test_outcome.test_id")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PredictionCorrectness {
    Aligned,
    UnderPredicted,
    OverPredicted,
    DivergentInClass,
    Surprise,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct PredictionProvenance {
    pub predictor_version: String,
    pub constellation_version: String,
    pub calibration_version: String,
    pub active_pointer: String,
    /// #798 / #699: which `TrainHealthSource` produced the `TrainHealthSummary`
    /// at inference time. SCREAMING_SNAKE form of the enum
    /// (`MEASURED` / `DIAGNOSTIC_CERTIFICATE_ONLY_NEUTRAL` /
    /// `BOOTSTRAP_NO_DATA`). Empty string only for legacy / not-yet-recorded
    /// rows; new rows always populate this field. Operators can read this to
    /// see *why* the confidence multiplier was 1.0 vs. real.
    #[serde(default)]
    pub train_health_source: String,
}

impl PredictionProvenance {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        validate_optional_id(
            "prediction_provenance.predictor_version",
            &self.predictor_version,
        )?;
        validate_optional_id(
            "prediction_provenance.constellation_version",
            &self.constellation_version,
        )?;
        validate_optional_id(
            "prediction_provenance.calibration_version",
            &self.calibration_version,
        )?;
        validate_optional_id("prediction_provenance.active_pointer", &self.active_pointer)?;
        validate_optional_id(
            "prediction_provenance.train_health_source",
            &self.train_health_source,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct PhaseBPredictionSurfaces {
    pub predicted_failure_modes: Vec<PredictedFailureMode>,
    pub predicted_failed_tests: Vec<PredictedTestOutcome>,
    pub predicted_works: Vec<PredictedWorks>,
    pub predicted_uncovered_paths: Vec<UncoveredPath>,
    pub predicted_flaky_tests: Vec<FlakyTestCandidate>,
    pub guard_violations: Vec<GuardViolation>,
    pub closest_exemplars: Vec<ExemplarMatch>,
    pub predicted_edge_cases: Vec<PredictedEdgeCase>,
    pub predicted_latent_bugs: Vec<PredictedLatentBug>,
    pub predicted_tech_debt_added: Vec<PredictedTechDebt>,
    pub predicted_dead_code: Vec<PredictedDeadCode>,
    pub predicted_redundant_code: Vec<PredictedRedundancy>,
    pub predicted_perf_regressions: Vec<PredictedPerfRegression>,
    pub predicted_security_concerns: Vec<PredictedSecurityConcern>,
    pub predicted_accuracy_degradations: Vec<PredictedAccuracyDegradation>,
    pub predicted_cost_regressions: Vec<PredictedCostRegression>,
    pub predicted_reasoning_class: ReasoningClass,
}

impl PhaseBPredictionSurfaces {
    pub fn clear_q4_display_only_fields(&mut self) {
        self.predicted_edge_cases.clear();
        self.predicted_latent_bugs.clear();
        self.predicted_tech_debt_added.clear();
        self.predicted_dead_code.clear();
        self.predicted_redundant_code.clear();
        self.predicted_perf_regressions.clear();
        self.predicted_security_concerns.clear();
        self.predicted_accuracy_degradations.clear();
        self.predicted_cost_regressions.clear();
        self.predicted_reasoning_class = ReasoningClass::Mute;
    }

    pub fn q4_display_only_field_count(&self) -> usize {
        self.predicted_edge_cases.len()
            + self.predicted_latent_bugs.len()
            + self.predicted_tech_debt_added.len()
            + self.predicted_dead_code.len()
            + self.predicted_redundant_code.len()
            + self.predicted_perf_regressions.len()
            + self.predicted_security_concerns.len()
            + self.predicted_accuracy_degradations.len()
            + self.predicted_cost_regressions.len()
            + usize::from(self.predicted_reasoning_class != ReasoningClass::Mute)
    }
}

pub const HIERARCHICAL_PREDICTION_SCHEMA_VERSION: u32 = 1;
pub const SLOT_ATTRIBUTION_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SlotAttributionPolarity {
    Supporting,
    Violating,
    Missing,
    Stale,
    Relationship,
    Q4Concern,
    Q5Impact,
}

impl SlotAttributionPolarity {
    fn is_rejection_evidence(self) -> bool {
        matches!(self, Self::Violating | Self::Missing | Self::Stale)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SlotAttributionSource {
    VerdictHead,
    PredictedWorks,
    FailureMode,
    GuardViolation,
    PerSlotOod,
    ConstellationPair,
    FailureFingerprint,
    ActiveLearningCandidate,
    Q4Head,
    Q5Replay,
    ClaimReconciliation,
    GrangerAttestation,
    AcceptedLabel,
    ConstellationSkill,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SlotAttributionEvidence {
    pub schema_version: u32,
    pub slot_id: String,
    pub embedder: Option<EmbedderId>,
    pub chunk: Option<ChunkId>,
    pub polarity: SlotAttributionPolarity,
    pub source: SlotAttributionSource,
    pub score: f32,
    pub threshold: Option<f32>,
    pub margin: Option<f32>,
    pub reason: String,
    pub relationship_slot_id: Option<String>,
    pub related_fingerprint_id: Option<String>,
    pub active_learning_candidate_id: Option<[u8; 16]>,
    pub q_head: Option<String>,
    pub impact_kind: Option<String>,
    pub evidence: String,
}

impl SlotAttributionEvidence {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        if self.schema_version != SLOT_ATTRIBUTION_SCHEMA_VERSION {
            return Err(MejepaInferError::InvalidInput {
                field: "slot_attribution.schema_version".to_string(),
                detail: format!(
                    "expected schema_version {}, got {}",
                    SLOT_ATTRIBUTION_SCHEMA_VERSION, self.schema_version
                ),
            });
        }
        validate_bounded_id("slot_attribution.slot_id", &self.slot_id, MAX_ID_BYTES)?;
        if let Some(embedder) = &self.embedder {
            embedder.validate("slot_attribution.embedder")?;
        }
        if let Some(chunk) = &self.chunk {
            chunk.validate("slot_attribution.chunk")?;
        }
        validate_probability("slot_attribution.score", self.score)?;
        validate_optional_probability("slot_attribution.threshold", self.threshold)?;
        if let Some(margin) = self.margin {
            validate_finite_f32("slot_attribution.margin", margin)?;
            if margin < 0.0 {
                return Err(MejepaInferError::InvalidInput {
                    field: "slot_attribution.margin".to_string(),
                    detail: "margin must be non-negative".to_string(),
                });
            }
        }
        validate_bounded_text(
            "slot_attribution.reason",
            &self.reason,
            MAX_COMMIT_MESSAGE_BYTES,
        )?;
        validate_bounded_text(
            "slot_attribution.evidence",
            &self.evidence,
            MAX_COMMIT_MESSAGE_BYTES,
        )?;
        if let Some(slot) = &self.relationship_slot_id {
            validate_bounded_id("slot_attribution.relationship_slot_id", slot, MAX_ID_BYTES)?;
            if slot == &self.slot_id {
                return Err(MejepaInferError::InvalidInput {
                    field: "slot_attribution.relationship_slot_id".to_string(),
                    detail: "relationship_slot_id must differ from slot_id".to_string(),
                });
            }
        }
        if let Some(fingerprint) = &self.related_fingerprint_id {
            validate_bounded_id(
                "slot_attribution.related_fingerprint_id",
                fingerprint,
                MAX_ID_BYTES,
            )?;
        }
        if let Some(candidate) = self.active_learning_candidate_id {
            if candidate.iter().all(|byte| *byte == 0) {
                return Err(MejepaInferError::InvalidInput {
                    field: "slot_attribution.active_learning_candidate_id".to_string(),
                    detail: "candidate id must be non-zero when present".to_string(),
                });
            }
        }
        if let Some(head) = &self.q_head {
            validate_bounded_id("slot_attribution.q_head", head, MAX_ID_BYTES)?;
        }
        if let Some(kind) = &self.impact_kind {
            validate_bounded_id("slot_attribution.impact_kind", kind, MAX_ID_BYTES)?;
        }
        Ok(())
    }
}

pub const PREDICTION_LABEL_CONTEXT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PredictionLabelContext {
    pub schema_version: u32,
    pub accepted_label_ids: Vec<String>,
    pub code_state_key: Option<String>,
    pub failure_evidence_set_ids: Vec<String>,
    pub active_skill_ids: Vec<String>,
    pub accepted_registry_sha256: Option<String>,
    pub usefulness_metrics_sha256: Option<String>,
    pub learning_bridge_manifest_sha256: Option<String>,
    pub label_signature_hash: Option<String>,
    pub skill_signature_hash: Option<String>,
    #[serde(default)]
    pub active_higher_ability_ids: Vec<String>,
    #[serde(default)]
    pub source_membership_keys: Vec<String>,
    #[serde(default)]
    pub ability_signature_hash: Option<String>,
    #[serde(default)]
    pub membership_signature_hash: Option<String>,
}

impl Default for PredictionLabelContext {
    fn default() -> Self {
        Self {
            schema_version: PREDICTION_LABEL_CONTEXT_SCHEMA_VERSION,
            accepted_label_ids: Vec::new(),
            code_state_key: None,
            failure_evidence_set_ids: Vec::new(),
            active_skill_ids: Vec::new(),
            active_higher_ability_ids: Vec::new(),
            source_membership_keys: Vec::new(),
            accepted_registry_sha256: None,
            usefulness_metrics_sha256: None,
            learning_bridge_manifest_sha256: None,
            label_signature_hash: None,
            skill_signature_hash: None,
            ability_signature_hash: None,
            membership_signature_hash: None,
        }
    }
}

impl PredictionLabelContext {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        if self.schema_version != PREDICTION_LABEL_CONTEXT_SCHEMA_VERSION {
            return Err(MejepaInferError::InvalidInput {
                field: "label_context.schema_version".to_string(),
                detail: format!(
                    "expected schema_version {}, got {}",
                    PREDICTION_LABEL_CONTEXT_SCHEMA_VERSION, self.schema_version
                ),
            });
        }
        validate_id_list(
            "label_context.accepted_label_ids",
            &self.accepted_label_ids,
            MAX_ACCEPTED_LABEL_IDS,
        )?;
        validate_id_list(
            "label_context.failure_evidence_set_ids",
            &self.failure_evidence_set_ids,
            MAX_FAILURE_EVIDENCE_SET_IDS,
        )?;
        validate_id_list(
            "label_context.active_skill_ids",
            &self.active_skill_ids,
            MAX_ACTIVE_SKILL_IDS,
        )?;
        validate_id_list(
            "label_context.active_higher_ability_ids",
            &self.active_higher_ability_ids,
            MAX_ACTIVE_HIGHER_ABILITY_IDS,
        )?;
        validate_id_list(
            "label_context.source_membership_keys",
            &self.source_membership_keys,
            MAX_SOURCE_MEMBERSHIP_KEYS,
        )?;
        if let Some(key) = &self.code_state_key {
            validate_bounded_id("label_context.code_state_key", key, MAX_ID_BYTES)?;
        }
        for (field, value) in [
            (
                "label_context.accepted_registry_sha256",
                &self.accepted_registry_sha256,
            ),
            (
                "label_context.usefulness_metrics_sha256",
                &self.usefulness_metrics_sha256,
            ),
            (
                "label_context.learning_bridge_manifest_sha256",
                &self.learning_bridge_manifest_sha256,
            ),
            (
                "label_context.label_signature_hash",
                &self.label_signature_hash,
            ),
            (
                "label_context.skill_signature_hash",
                &self.skill_signature_hash,
            ),
            (
                "label_context.ability_signature_hash",
                &self.ability_signature_hash,
            ),
            (
                "label_context.membership_signature_hash",
                &self.membership_signature_hash,
            ),
        ] {
            if let Some(value) = value {
                validate_bounded_id(field, value, MAX_ID_BYTES)?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PredictionLabelContextLegacyV1 {
    schema_version: u32,
    accepted_label_ids: Vec<String>,
    code_state_key: Option<String>,
    failure_evidence_set_ids: Vec<String>,
    active_skill_ids: Vec<String>,
    accepted_registry_sha256: Option<String>,
    usefulness_metrics_sha256: Option<String>,
    learning_bridge_manifest_sha256: Option<String>,
    label_signature_hash: Option<String>,
    skill_signature_hash: Option<String>,
}

impl PredictionLabelContextLegacyV1 {
    #[cfg(test)]
    fn from_current(value: &PredictionLabelContext) -> Self {
        Self {
            schema_version: value.schema_version,
            accepted_label_ids: value.accepted_label_ids.clone(),
            code_state_key: value.code_state_key.clone(),
            failure_evidence_set_ids: value.failure_evidence_set_ids.clone(),
            active_skill_ids: value.active_skill_ids.clone(),
            accepted_registry_sha256: value.accepted_registry_sha256.clone(),
            usefulness_metrics_sha256: value.usefulness_metrics_sha256.clone(),
            learning_bridge_manifest_sha256: value.learning_bridge_manifest_sha256.clone(),
            label_signature_hash: value.label_signature_hash.clone(),
            skill_signature_hash: value.skill_signature_hash.clone(),
        }
    }

    fn into_current(self) -> PredictionLabelContext {
        PredictionLabelContext {
            schema_version: self.schema_version,
            accepted_label_ids: self.accepted_label_ids,
            code_state_key: self.code_state_key,
            failure_evidence_set_ids: self.failure_evidence_set_ids,
            active_skill_ids: self.active_skill_ids,
            accepted_registry_sha256: self.accepted_registry_sha256,
            usefulness_metrics_sha256: self.usefulness_metrics_sha256,
            learning_bridge_manifest_sha256: self.learning_bridge_manifest_sha256,
            label_signature_hash: self.label_signature_hash,
            skill_signature_hash: self.skill_signature_hash,
            active_higher_ability_ids: Vec::new(),
            source_membership_keys: Vec::new(),
            ability_signature_hash: None,
            membership_signature_hash: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PerSlotOodReasonKind {
    SlotThresholdExceeded,
    DiffuseSlotThresholdExceeded,
    MissingSlotObservation,
    StaleCalibration,
    InvalidCalibration,
    NonFiniteSlotScore,
    ConformalIntervalTooWide,
    GtauViolation,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PerSlotOodReason {
    pub embedder: EmbedderId,
    pub chunk: Option<ChunkId>,
    pub reason: PerSlotOodReasonKind,
    pub observed_score: f32,
    pub threshold: f32,
    pub margin: f32,
    pub calibration_version: String,
    pub evidence: String,
}

impl PerSlotOodReason {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        self.embedder.validate("per_slot_ood_reason.embedder")?;
        if let Some(chunk) = &self.chunk {
            chunk.validate("per_slot_ood_reason.chunk")?;
        }
        validate_finite_f32("per_slot_ood_reason.observed_score", self.observed_score)?;
        validate_finite_f32("per_slot_ood_reason.threshold", self.threshold)?;
        validate_finite_f32("per_slot_ood_reason.margin", self.margin)?;
        if self.margin < 0.0 {
            return Err(MejepaInferError::InvalidInput {
                field: "per_slot_ood_reason.margin".to_string(),
                detail: "margin must be non-negative".to_string(),
            });
        }
        validate_non_empty_id(
            "per_slot_ood_reason.calibration_version",
            &self.calibration_version,
        )?;
        validate_bounded_text(
            "per_slot_ood_reason.evidence",
            &self.evidence,
            MAX_COMMIT_MESSAGE_BYTES,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PredictionHierarchyLevel {
    File,
    Function,
    AstNode,
    Chunk,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HierarchicalPredictionLevel {
    pub level: PredictionHierarchyLevel,
    pub scope_id: String,
    pub parent_scope_id: Option<String>,
    pub covered_chunks: Vec<ChunkId>,
    pub predicted_oracle_pass: f32,
    pub calibrated_confidence: f32,
    pub ood_score: f32,
    pub verdict: Verdict,
    pub latent_energy: f32,
}

impl HierarchicalPredictionLevel {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        validate_bounded_id(
            "hierarchical_prediction.level.scope_id",
            &self.scope_id,
            MAX_HIERARCHY_SCOPE_ID_BYTES,
        )?;
        if let Some(parent) = &self.parent_scope_id {
            validate_bounded_id(
                "hierarchical_prediction.level.parent_scope_id",
                parent,
                MAX_HIERARCHY_SCOPE_ID_BYTES,
            )?;
        }
        if self.covered_chunks.is_empty() {
            return Err(MejepaInferError::InvalidInput {
                field: "hierarchical_prediction.level.covered_chunks".to_string(),
                detail: "covered_chunks must be non-empty".to_string(),
            });
        }
        if self.covered_chunks.len() > MAX_CHUNKS {
            return Err(MejepaInferError::DimMismatch {
                expected: MAX_CHUNKS,
                actual: self.covered_chunks.len(),
                context: "hierarchical_prediction.level.covered_chunks exceeds maximum".to_string(),
            });
        }
        for (idx, chunk) in self.covered_chunks.iter().enumerate() {
            chunk.validate(&format!(
                "hierarchical_prediction.level.covered_chunks[{idx}]"
            ))?;
        }
        validate_probability(
            "hierarchical_prediction.level.predicted_oracle_pass",
            self.predicted_oracle_pass,
        )?;
        validate_probability(
            "hierarchical_prediction.level.calibrated_confidence",
            self.calibrated_confidence,
        )?;
        validate_probability("hierarchical_prediction.level.ood_score", self.ood_score)?;
        validate_finite_f32(
            "hierarchical_prediction.level.latent_energy",
            self.latent_energy,
        )?;
        if self.latent_energy < 0.0 {
            return Err(MejepaInferError::InvalidInput {
                field: "hierarchical_prediction.level.latent_energy".to_string(),
                detail: "latent_energy must be non-negative".to_string(),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HierarchicalPredictionRecord {
    pub schema_version: u32,
    pub prediction_id: [u8; 16],
    pub task_id: TaskId,
    pub session_id: [u8; 16],
    pub language: Language,
    pub source_panel_sha: [u8; 32],
    pub calibration_version: String,
    pub created_at_unix_ms: i64,
    #[serde(default)]
    pub slot_attributions: Vec<SlotAttributionEvidence>,
    pub levels: Vec<HierarchicalPredictionLevel>,
}

impl HierarchicalPredictionRecord {
    pub fn try_new(mut value: Self) -> Result<Self, MejepaInferError> {
        value.validate()?;
        value.levels.shrink_to_fit();
        Ok(value)
    }

    pub fn validate(&self) -> Result<(), MejepaInferError> {
        if self.schema_version != HIERARCHICAL_PREDICTION_SCHEMA_VERSION {
            return Err(MejepaInferError::InvalidInput {
                field: "hierarchical_prediction.schema_version".to_string(),
                detail: format!(
                    "expected schema_version {}, got {}",
                    HIERARCHICAL_PREDICTION_SCHEMA_VERSION, self.schema_version
                ),
            });
        }
        if self.prediction_id.iter().all(|byte| *byte == 0) {
            return Err(MejepaInferError::InvalidInput {
                field: "hierarchical_prediction.prediction_id".to_string(),
                detail: "prediction_id must be non-zero".to_string(),
            });
        }
        self.task_id.validate("hierarchical_prediction.task_id")?;
        if self.session_id == [0u8; 16] {
            return Err(MejepaInferError::InvalidInput {
                field: "hierarchical_prediction.session_id".to_string(),
                detail: "session_id must be non-zero".to_string(),
            });
        }
        validate_non_empty_id(
            "hierarchical_prediction.calibration_version",
            &self.calibration_version,
        )?;
        if self.created_at_unix_ms < 0 {
            return Err(MejepaInferError::InvalidInput {
                field: "hierarchical_prediction.created_at_unix_ms".to_string(),
                detail: "created_at_unix_ms must be non-negative".to_string(),
            });
        }
        validate_slot_attribution_collection(
            "hierarchical_prediction.slot_attributions",
            &self.slot_attributions,
            None,
        )?;
        if self.levels.len() > MAX_HIERARCHY_LEVELS {
            return Err(MejepaInferError::DimMismatch {
                expected: MAX_HIERARCHY_LEVELS,
                actual: self.levels.len(),
                context: "hierarchical_prediction.levels exceeds maximum".to_string(),
            });
        }
        validate_items(
            "hierarchical_prediction.levels",
            &self.levels,
            HierarchicalPredictionLevel::validate,
        )?;
        let mut seen_scopes = BTreeSet::new();
        let mut seen_levels = BTreeSet::new();
        for level in &self.levels {
            if !seen_scopes.insert(level.scope_id.as_str()) {
                return Err(MejepaInferError::InvalidInput {
                    field: "hierarchical_prediction.level.scope_id".to_string(),
                    detail: format!("duplicate scope_id {}", level.scope_id),
                });
            }
            seen_levels.insert(level.level);
        }
        for required in [
            PredictionHierarchyLevel::File,
            PredictionHierarchyLevel::Function,
            PredictionHierarchyLevel::AstNode,
            PredictionHierarchyLevel::Chunk,
        ] {
            if !seen_levels.contains(&required) {
                return Err(MejepaInferError::InvalidInput {
                    field: "hierarchical_prediction.levels".to_string(),
                    detail: format!("missing required {required:?} hierarchy level"),
                });
            }
        }
        for level in &self.levels {
            match (&level.level, &level.parent_scope_id) {
                (PredictionHierarchyLevel::File, None) => {}
                (PredictionHierarchyLevel::File, Some(_)) => {
                    return Err(MejepaInferError::InvalidInput {
                        field: "hierarchical_prediction.level.parent_scope_id".to_string(),
                        detail: "file levels must not have a parent_scope_id".to_string(),
                    });
                }
                (_, Some(parent)) if seen_scopes.contains(parent.as_str()) => {}
                (_, Some(parent)) => {
                    return Err(MejepaInferError::InvalidInput {
                        field: "hierarchical_prediction.level.parent_scope_id".to_string(),
                        detail: format!("parent scope {parent} does not exist in record"),
                    });
                }
                (_, None) => {
                    return Err(MejepaInferError::InvalidInput {
                        field: "hierarchical_prediction.level.parent_scope_id".to_string(),
                        detail: "non-file levels must have a parent_scope_id".to_string(),
                    });
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HierarchicalPredictionRecordLegacyNoSlotAttributions {
    schema_version: u32,
    prediction_id: [u8; 16],
    task_id: TaskId,
    session_id: [u8; 16],
    language: Language,
    source_panel_sha: [u8; 32],
    calibration_version: String,
    created_at_unix_ms: i64,
    levels: Vec<HierarchicalPredictionLevel>,
}

impl HierarchicalPredictionRecordLegacyNoSlotAttributions {
    #[cfg(test)]
    fn from_current(value: &HierarchicalPredictionRecord) -> Self {
        Self {
            schema_version: value.schema_version,
            prediction_id: value.prediction_id,
            task_id: value.task_id.clone(),
            session_id: value.session_id,
            language: value.language,
            source_panel_sha: value.source_panel_sha,
            calibration_version: value.calibration_version.clone(),
            created_at_unix_ms: value.created_at_unix_ms,
            levels: value.levels.clone(),
        }
    }

    fn into_current(self) -> Result<HierarchicalPredictionRecord, MejepaInferError> {
        HierarchicalPredictionRecord::try_new(HierarchicalPredictionRecord {
            schema_version: self.schema_version,
            prediction_id: self.prediction_id,
            task_id: self.task_id,
            session_id: self.session_id,
            language: self.language,
            source_panel_sha: self.source_panel_sha,
            calibration_version: self.calibration_version,
            created_at_unix_ms: self.created_at_unix_ms,
            slot_attributions: Vec::new(),
            levels: self.levels,
        })
    }
}

pub fn decode_hierarchical_prediction_record(
    bytes: &[u8],
) -> Result<HierarchicalPredictionRecord, MejepaInferError> {
    match bincode::deserialize::<HierarchicalPredictionRecord>(bytes) {
        Ok(record) => HierarchicalPredictionRecord::try_new(record),
        Err(current_err) => {
            let legacy =
                bincode::deserialize::<HierarchicalPredictionRecordLegacyNoSlotAttributions>(bytes)
                    .map_err(|legacy_err| MejepaInferError::InvalidInput {
                        field: "HierarchicalPredictionRecord.bincode".to_string(),
                        detail: format!(
                            "failed to decode current schema ({current_err}) or legacy no-slot-attributions schema ({legacy_err})"
                        ),
                    })?;
            legacy.into_current()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RealityPrediction {
    pub prediction_id: [u8; 16],
    pub witness_hash: WitnessHash,
    pub task_id: TaskId,
    pub session_id: [u8; 16],
    pub language: Language,
    pub covered_chunks: Vec<ChunkId>,
    pub verdict: Verdict,
    pub confidence_interval: ConformalInterval,
    pub predicted_oracle_pass: f32,
    pub predicted_test_pass: Vec<f32>,
    pub predicted_runtime_trace: [f32; 32],
    pub ood_score: f32,
    pub outcome_set: ConformalSet,
    pub calibrated_confidence: f32,
    pub degraded_status: bool,
    pub granger_attestations: BTreeMap<String, f32>,
    pub predicted_failure_modes: Vec<PredictedFailureMode>,
    pub predicted_failed_tests: Vec<PredictedTestOutcome>,
    #[serde(default)]
    pub predicted_works: Vec<PredictedWorks>,
    #[serde(default)]
    pub predicted_uncovered_paths: Vec<UncoveredPath>,
    #[serde(default)]
    pub predicted_flaky_tests: Vec<FlakyTestCandidate>,
    pub guard_violations: Vec<GuardViolation>,
    #[serde(default)]
    pub per_slot_ood_reasons: Vec<PerSlotOodReason>,
    pub closest_exemplars: Vec<ExemplarMatch>,
    pub predicted_edge_cases: Vec<PredictedEdgeCase>,
    pub predicted_latent_bugs: Vec<PredictedLatentBug>,
    pub predicted_tech_debt_added: Vec<PredictedTechDebt>,
    pub predicted_dead_code: Vec<PredictedDeadCode>,
    pub predicted_redundant_code: Vec<PredictedRedundancy>,
    pub predicted_perf_regressions: Vec<PredictedPerfRegression>,
    pub predicted_security_concerns: Vec<PredictedSecurityConcern>,
    pub predicted_accuracy_degradations: Vec<PredictedAccuracyDegradation>,
    pub predicted_cost_regressions: Vec<PredictedCostRegression>,
    pub predicted_reasoning_class: ReasoningClass,
    pub agent_claim_graph: AgentClaimGraph,
    pub claim_reconciliation: Vec<ClaimReconciliation>,
    pub reality_impact: Option<RealityImpact>,
    pub provenance: PredictionProvenance,
    pub source_panel_sha: [u8; 32],
    pub calibration_version: String,
    pub created_at_unix_ms: i64,
    // TASK-FP-010 (#319) — failure-shape fingerprint catalog evidence.
    // `serde(default)` so bincode-stored predictions written before this field
    // landed deserialize cleanly with `None` (additive, no schema bump).
    #[serde(default)]
    pub matched_fingerprint: Option<MatchedFingerprintEvidence>,
    #[serde(default)]
    pub unknown_candidate_id: Option<[u8; 16]>,
    #[serde(default)]
    pub constellation_intelligence: Option<ConstellationIntelligenceEvidence>,
    #[serde(default)]
    pub slot_attributions: Vec<SlotAttributionEvidence>,
    #[serde(default)]
    pub label_context: PredictionLabelContext,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RealityPredictionLegacyNoLabelContext {
    prediction_id: [u8; 16],
    witness_hash: WitnessHash,
    task_id: TaskId,
    session_id: [u8; 16],
    language: Language,
    covered_chunks: Vec<ChunkId>,
    verdict: Verdict,
    confidence_interval: ConformalInterval,
    predicted_oracle_pass: f32,
    predicted_test_pass: Vec<f32>,
    predicted_runtime_trace: [f32; 32],
    ood_score: f32,
    outcome_set: ConformalSet,
    calibrated_confidence: f32,
    degraded_status: bool,
    granger_attestations: BTreeMap<String, f32>,
    predicted_failure_modes: Vec<PredictedFailureMode>,
    predicted_failed_tests: Vec<PredictedTestOutcome>,
    predicted_works: Vec<PredictedWorks>,
    predicted_uncovered_paths: Vec<UncoveredPath>,
    predicted_flaky_tests: Vec<FlakyTestCandidate>,
    guard_violations: Vec<GuardViolation>,
    per_slot_ood_reasons: Vec<PerSlotOodReason>,
    closest_exemplars: Vec<ExemplarMatch>,
    predicted_edge_cases: Vec<PredictedEdgeCase>,
    predicted_latent_bugs: Vec<PredictedLatentBug>,
    predicted_tech_debt_added: Vec<PredictedTechDebt>,
    predicted_dead_code: Vec<PredictedDeadCode>,
    predicted_redundant_code: Vec<PredictedRedundancy>,
    predicted_perf_regressions: Vec<PredictedPerfRegression>,
    predicted_security_concerns: Vec<PredictedSecurityConcern>,
    predicted_accuracy_degradations: Vec<PredictedAccuracyDegradation>,
    predicted_cost_regressions: Vec<PredictedCostRegression>,
    predicted_reasoning_class: ReasoningClass,
    agent_claim_graph: AgentClaimGraph,
    claim_reconciliation: Vec<ClaimReconciliation>,
    reality_impact: Option<RealityImpact>,
    provenance: PredictionProvenance,
    source_panel_sha: [u8; 32],
    calibration_version: String,
    created_at_unix_ms: i64,
    matched_fingerprint: Option<MatchedFingerprintEvidence>,
    unknown_candidate_id: Option<[u8; 16]>,
    constellation_intelligence: Option<ConstellationIntelligenceEvidence>,
    slot_attributions: Vec<SlotAttributionEvidence>,
}

impl RealityPredictionLegacyNoLabelContext {
    #[cfg(test)]
    fn from_current(value: &RealityPrediction) -> Self {
        Self {
            prediction_id: value.prediction_id,
            witness_hash: value.witness_hash,
            task_id: value.task_id.clone(),
            session_id: value.session_id,
            language: value.language,
            covered_chunks: value.covered_chunks.clone(),
            verdict: value.verdict,
            confidence_interval: value.confidence_interval,
            predicted_oracle_pass: value.predicted_oracle_pass,
            predicted_test_pass: value.predicted_test_pass.clone(),
            predicted_runtime_trace: value.predicted_runtime_trace,
            ood_score: value.ood_score,
            outcome_set: value.outcome_set.clone(),
            calibrated_confidence: value.calibrated_confidence,
            degraded_status: value.degraded_status,
            granger_attestations: value.granger_attestations.clone(),
            predicted_failure_modes: value.predicted_failure_modes.clone(),
            predicted_failed_tests: value.predicted_failed_tests.clone(),
            predicted_works: value.predicted_works.clone(),
            predicted_uncovered_paths: value.predicted_uncovered_paths.clone(),
            predicted_flaky_tests: value.predicted_flaky_tests.clone(),
            guard_violations: value.guard_violations.clone(),
            per_slot_ood_reasons: value.per_slot_ood_reasons.clone(),
            closest_exemplars: value.closest_exemplars.clone(),
            predicted_edge_cases: value.predicted_edge_cases.clone(),
            predicted_latent_bugs: value.predicted_latent_bugs.clone(),
            predicted_tech_debt_added: value.predicted_tech_debt_added.clone(),
            predicted_dead_code: value.predicted_dead_code.clone(),
            predicted_redundant_code: value.predicted_redundant_code.clone(),
            predicted_perf_regressions: value.predicted_perf_regressions.clone(),
            predicted_security_concerns: value.predicted_security_concerns.clone(),
            predicted_accuracy_degradations: value.predicted_accuracy_degradations.clone(),
            predicted_cost_regressions: value.predicted_cost_regressions.clone(),
            predicted_reasoning_class: value.predicted_reasoning_class,
            agent_claim_graph: value.agent_claim_graph.clone(),
            claim_reconciliation: value.claim_reconciliation.clone(),
            reality_impact: value.reality_impact.clone(),
            provenance: value.provenance.clone(),
            source_panel_sha: value.source_panel_sha,
            calibration_version: value.calibration_version.clone(),
            created_at_unix_ms: value.created_at_unix_ms,
            matched_fingerprint: value.matched_fingerprint.clone(),
            unknown_candidate_id: value.unknown_candidate_id,
            constellation_intelligence: value.constellation_intelligence.clone(),
            slot_attributions: value.slot_attributions.clone(),
        }
    }

    fn into_current(self) -> Result<RealityPrediction, MejepaInferError> {
        self.into_current_with_label_context(PredictionLabelContext::default())
    }

    fn into_current_with_label_context(
        self,
        label_context: PredictionLabelContext,
    ) -> Result<RealityPrediction, MejepaInferError> {
        RealityPrediction::try_new(RealityPrediction {
            prediction_id: self.prediction_id,
            witness_hash: self.witness_hash,
            task_id: self.task_id,
            session_id: self.session_id,
            language: self.language,
            covered_chunks: self.covered_chunks,
            verdict: self.verdict,
            confidence_interval: self.confidence_interval,
            predicted_oracle_pass: self.predicted_oracle_pass,
            predicted_test_pass: self.predicted_test_pass,
            predicted_runtime_trace: self.predicted_runtime_trace,
            ood_score: self.ood_score,
            outcome_set: self.outcome_set,
            calibrated_confidence: self.calibrated_confidence,
            degraded_status: self.degraded_status,
            granger_attestations: self.granger_attestations,
            predicted_failure_modes: self.predicted_failure_modes,
            predicted_failed_tests: self.predicted_failed_tests,
            predicted_works: self.predicted_works,
            predicted_uncovered_paths: self.predicted_uncovered_paths,
            predicted_flaky_tests: self.predicted_flaky_tests,
            guard_violations: self.guard_violations,
            per_slot_ood_reasons: self.per_slot_ood_reasons,
            closest_exemplars: self.closest_exemplars,
            predicted_edge_cases: self.predicted_edge_cases,
            predicted_latent_bugs: self.predicted_latent_bugs,
            predicted_tech_debt_added: self.predicted_tech_debt_added,
            predicted_dead_code: self.predicted_dead_code,
            predicted_redundant_code: self.predicted_redundant_code,
            predicted_perf_regressions: self.predicted_perf_regressions,
            predicted_security_concerns: self.predicted_security_concerns,
            predicted_accuracy_degradations: self.predicted_accuracy_degradations,
            predicted_cost_regressions: self.predicted_cost_regressions,
            predicted_reasoning_class: self.predicted_reasoning_class,
            agent_claim_graph: self.agent_claim_graph,
            claim_reconciliation: self.claim_reconciliation,
            reality_impact: self.reality_impact,
            provenance: self.provenance,
            source_panel_sha: self.source_panel_sha,
            calibration_version: self.calibration_version,
            created_at_unix_ms: self.created_at_unix_ms,
            matched_fingerprint: self.matched_fingerprint,
            unknown_candidate_id: self.unknown_candidate_id,
            constellation_intelligence: self.constellation_intelligence,
            slot_attributions: self.slot_attributions,
            label_context,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RealityPredictionLegacyNoSlotAttributions {
    prediction_id: [u8; 16],
    witness_hash: WitnessHash,
    task_id: TaskId,
    session_id: [u8; 16],
    language: Language,
    covered_chunks: Vec<ChunkId>,
    verdict: Verdict,
    confidence_interval: ConformalInterval,
    predicted_oracle_pass: f32,
    predicted_test_pass: Vec<f32>,
    predicted_runtime_trace: [f32; 32],
    ood_score: f32,
    outcome_set: ConformalSet,
    calibrated_confidence: f32,
    degraded_status: bool,
    granger_attestations: BTreeMap<String, f32>,
    predicted_failure_modes: Vec<PredictedFailureMode>,
    predicted_failed_tests: Vec<PredictedTestOutcome>,
    predicted_works: Vec<PredictedWorks>,
    predicted_uncovered_paths: Vec<UncoveredPath>,
    predicted_flaky_tests: Vec<FlakyTestCandidate>,
    guard_violations: Vec<GuardViolation>,
    per_slot_ood_reasons: Vec<PerSlotOodReason>,
    closest_exemplars: Vec<ExemplarMatch>,
    predicted_edge_cases: Vec<PredictedEdgeCase>,
    predicted_latent_bugs: Vec<PredictedLatentBug>,
    predicted_tech_debt_added: Vec<PredictedTechDebt>,
    predicted_dead_code: Vec<PredictedDeadCode>,
    predicted_redundant_code: Vec<PredictedRedundancy>,
    predicted_perf_regressions: Vec<PredictedPerfRegression>,
    predicted_security_concerns: Vec<PredictedSecurityConcern>,
    predicted_accuracy_degradations: Vec<PredictedAccuracyDegradation>,
    predicted_cost_regressions: Vec<PredictedCostRegression>,
    predicted_reasoning_class: ReasoningClass,
    agent_claim_graph: AgentClaimGraph,
    claim_reconciliation: Vec<ClaimReconciliation>,
    reality_impact: Option<RealityImpact>,
    provenance: PredictionProvenance,
    source_panel_sha: [u8; 32],
    calibration_version: String,
    created_at_unix_ms: i64,
    matched_fingerprint: Option<MatchedFingerprintEvidence>,
    unknown_candidate_id: Option<[u8; 16]>,
    constellation_intelligence: Option<ConstellationIntelligenceEvidence>,
}

impl RealityPredictionLegacyNoSlotAttributions {
    #[cfg(test)]
    fn from_current(value: &RealityPrediction) -> Self {
        Self {
            prediction_id: value.prediction_id,
            witness_hash: value.witness_hash,
            task_id: value.task_id.clone(),
            session_id: value.session_id,
            language: value.language,
            covered_chunks: value.covered_chunks.clone(),
            verdict: value.verdict,
            confidence_interval: value.confidence_interval,
            predicted_oracle_pass: value.predicted_oracle_pass,
            predicted_test_pass: value.predicted_test_pass.clone(),
            predicted_runtime_trace: value.predicted_runtime_trace,
            ood_score: value.ood_score,
            outcome_set: value.outcome_set.clone(),
            calibrated_confidence: value.calibrated_confidence,
            degraded_status: value.degraded_status,
            granger_attestations: value.granger_attestations.clone(),
            predicted_failure_modes: value.predicted_failure_modes.clone(),
            predicted_failed_tests: value.predicted_failed_tests.clone(),
            predicted_works: value.predicted_works.clone(),
            predicted_uncovered_paths: value.predicted_uncovered_paths.clone(),
            predicted_flaky_tests: value.predicted_flaky_tests.clone(),
            guard_violations: value.guard_violations.clone(),
            per_slot_ood_reasons: value.per_slot_ood_reasons.clone(),
            closest_exemplars: value.closest_exemplars.clone(),
            predicted_edge_cases: value.predicted_edge_cases.clone(),
            predicted_latent_bugs: value.predicted_latent_bugs.clone(),
            predicted_tech_debt_added: value.predicted_tech_debt_added.clone(),
            predicted_dead_code: value.predicted_dead_code.clone(),
            predicted_redundant_code: value.predicted_redundant_code.clone(),
            predicted_perf_regressions: value.predicted_perf_regressions.clone(),
            predicted_security_concerns: value.predicted_security_concerns.clone(),
            predicted_accuracy_degradations: value.predicted_accuracy_degradations.clone(),
            predicted_cost_regressions: value.predicted_cost_regressions.clone(),
            predicted_reasoning_class: value.predicted_reasoning_class,
            agent_claim_graph: value.agent_claim_graph.clone(),
            claim_reconciliation: value.claim_reconciliation.clone(),
            reality_impact: value.reality_impact.clone(),
            provenance: value.provenance.clone(),
            source_panel_sha: value.source_panel_sha,
            calibration_version: value.calibration_version.clone(),
            created_at_unix_ms: value.created_at_unix_ms,
            matched_fingerprint: value.matched_fingerprint.clone(),
            unknown_candidate_id: value.unknown_candidate_id,
            constellation_intelligence: value.constellation_intelligence.clone(),
        }
    }

    fn into_current(self) -> Result<RealityPrediction, MejepaInferError> {
        RealityPrediction::try_new(RealityPrediction {
            prediction_id: self.prediction_id,
            witness_hash: self.witness_hash,
            task_id: self.task_id,
            session_id: self.session_id,
            language: self.language,
            covered_chunks: self.covered_chunks,
            verdict: self.verdict,
            confidence_interval: self.confidence_interval,
            predicted_oracle_pass: self.predicted_oracle_pass,
            predicted_test_pass: self.predicted_test_pass,
            predicted_runtime_trace: self.predicted_runtime_trace,
            ood_score: self.ood_score,
            outcome_set: self.outcome_set,
            calibrated_confidence: self.calibrated_confidence,
            degraded_status: self.degraded_status,
            granger_attestations: self.granger_attestations,
            predicted_failure_modes: self.predicted_failure_modes,
            predicted_failed_tests: self.predicted_failed_tests,
            predicted_works: self.predicted_works,
            predicted_uncovered_paths: self.predicted_uncovered_paths,
            predicted_flaky_tests: self.predicted_flaky_tests,
            guard_violations: self.guard_violations,
            per_slot_ood_reasons: self.per_slot_ood_reasons,
            closest_exemplars: self.closest_exemplars,
            predicted_edge_cases: self.predicted_edge_cases,
            predicted_latent_bugs: self.predicted_latent_bugs,
            predicted_tech_debt_added: self.predicted_tech_debt_added,
            predicted_dead_code: self.predicted_dead_code,
            predicted_redundant_code: self.predicted_redundant_code,
            predicted_perf_regressions: self.predicted_perf_regressions,
            predicted_security_concerns: self.predicted_security_concerns,
            predicted_accuracy_degradations: self.predicted_accuracy_degradations,
            predicted_cost_regressions: self.predicted_cost_regressions,
            predicted_reasoning_class: self.predicted_reasoning_class,
            agent_claim_graph: self.agent_claim_graph,
            claim_reconciliation: self.claim_reconciliation,
            reality_impact: self.reality_impact,
            provenance: self.provenance,
            source_panel_sha: self.source_panel_sha,
            calibration_version: self.calibration_version,
            created_at_unix_ms: self.created_at_unix_ms,
            matched_fingerprint: self.matched_fingerprint,
            unknown_candidate_id: self.unknown_candidate_id,
            constellation_intelligence: self.constellation_intelligence,
            slot_attributions: Vec::new(),
            label_context: PredictionLabelContext::default(),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RealityPredictionLegacyNoConstellation {
    prediction_id: [u8; 16],
    witness_hash: WitnessHash,
    task_id: TaskId,
    session_id: [u8; 16],
    language: Language,
    covered_chunks: Vec<ChunkId>,
    verdict: Verdict,
    confidence_interval: ConformalInterval,
    predicted_oracle_pass: f32,
    predicted_test_pass: Vec<f32>,
    predicted_runtime_trace: [f32; 32],
    ood_score: f32,
    outcome_set: ConformalSet,
    calibrated_confidence: f32,
    degraded_status: bool,
    granger_attestations: BTreeMap<String, f32>,
    predicted_failure_modes: Vec<PredictedFailureMode>,
    predicted_failed_tests: Vec<PredictedTestOutcome>,
    predicted_works: Vec<PredictedWorks>,
    predicted_uncovered_paths: Vec<UncoveredPath>,
    predicted_flaky_tests: Vec<FlakyTestCandidate>,
    guard_violations: Vec<GuardViolation>,
    per_slot_ood_reasons: Vec<PerSlotOodReason>,
    closest_exemplars: Vec<ExemplarMatch>,
    predicted_edge_cases: Vec<PredictedEdgeCase>,
    predicted_latent_bugs: Vec<PredictedLatentBug>,
    predicted_tech_debt_added: Vec<PredictedTechDebt>,
    predicted_dead_code: Vec<PredictedDeadCode>,
    predicted_redundant_code: Vec<PredictedRedundancy>,
    predicted_perf_regressions: Vec<PredictedPerfRegression>,
    predicted_security_concerns: Vec<PredictedSecurityConcern>,
    predicted_accuracy_degradations: Vec<PredictedAccuracyDegradation>,
    predicted_cost_regressions: Vec<PredictedCostRegression>,
    predicted_reasoning_class: ReasoningClass,
    agent_claim_graph: AgentClaimGraph,
    claim_reconciliation: Vec<ClaimReconciliation>,
    reality_impact: Option<RealityImpact>,
    provenance: PredictionProvenance,
    source_panel_sha: [u8; 32],
    calibration_version: String,
    created_at_unix_ms: i64,
    matched_fingerprint: Option<MatchedFingerprintEvidence>,
    unknown_candidate_id: Option<[u8; 16]>,
}

impl RealityPredictionLegacyNoConstellation {
    #[cfg(test)]
    fn from_current(value: &RealityPrediction) -> Self {
        Self {
            prediction_id: value.prediction_id,
            witness_hash: value.witness_hash,
            task_id: value.task_id.clone(),
            session_id: value.session_id,
            language: value.language,
            covered_chunks: value.covered_chunks.clone(),
            verdict: value.verdict,
            confidence_interval: value.confidence_interval,
            predicted_oracle_pass: value.predicted_oracle_pass,
            predicted_test_pass: value.predicted_test_pass.clone(),
            predicted_runtime_trace: value.predicted_runtime_trace,
            ood_score: value.ood_score,
            outcome_set: value.outcome_set.clone(),
            calibrated_confidence: value.calibrated_confidence,
            degraded_status: value.degraded_status,
            granger_attestations: value.granger_attestations.clone(),
            predicted_failure_modes: value.predicted_failure_modes.clone(),
            predicted_failed_tests: value.predicted_failed_tests.clone(),
            predicted_works: value.predicted_works.clone(),
            predicted_uncovered_paths: value.predicted_uncovered_paths.clone(),
            predicted_flaky_tests: value.predicted_flaky_tests.clone(),
            guard_violations: value.guard_violations.clone(),
            per_slot_ood_reasons: value.per_slot_ood_reasons.clone(),
            closest_exemplars: value.closest_exemplars.clone(),
            predicted_edge_cases: value.predicted_edge_cases.clone(),
            predicted_latent_bugs: value.predicted_latent_bugs.clone(),
            predicted_tech_debt_added: value.predicted_tech_debt_added.clone(),
            predicted_dead_code: value.predicted_dead_code.clone(),
            predicted_redundant_code: value.predicted_redundant_code.clone(),
            predicted_perf_regressions: value.predicted_perf_regressions.clone(),
            predicted_security_concerns: value.predicted_security_concerns.clone(),
            predicted_accuracy_degradations: value.predicted_accuracy_degradations.clone(),
            predicted_cost_regressions: value.predicted_cost_regressions.clone(),
            predicted_reasoning_class: value.predicted_reasoning_class,
            agent_claim_graph: value.agent_claim_graph.clone(),
            claim_reconciliation: value.claim_reconciliation.clone(),
            reality_impact: value.reality_impact.clone(),
            provenance: value.provenance.clone(),
            source_panel_sha: value.source_panel_sha,
            calibration_version: value.calibration_version.clone(),
            created_at_unix_ms: value.created_at_unix_ms,
            matched_fingerprint: value.matched_fingerprint.clone(),
            unknown_candidate_id: value.unknown_candidate_id,
        }
    }

    fn into_current(self) -> Result<RealityPrediction, MejepaInferError> {
        RealityPrediction::try_new(RealityPrediction {
            prediction_id: self.prediction_id,
            witness_hash: self.witness_hash,
            task_id: self.task_id,
            session_id: self.session_id,
            language: self.language,
            covered_chunks: self.covered_chunks,
            verdict: self.verdict,
            confidence_interval: self.confidence_interval,
            predicted_oracle_pass: self.predicted_oracle_pass,
            predicted_test_pass: self.predicted_test_pass,
            predicted_runtime_trace: self.predicted_runtime_trace,
            ood_score: self.ood_score,
            outcome_set: self.outcome_set,
            calibrated_confidence: self.calibrated_confidence,
            degraded_status: self.degraded_status,
            granger_attestations: self.granger_attestations,
            predicted_failure_modes: self.predicted_failure_modes,
            predicted_failed_tests: self.predicted_failed_tests,
            predicted_works: self.predicted_works,
            predicted_uncovered_paths: self.predicted_uncovered_paths,
            predicted_flaky_tests: self.predicted_flaky_tests,
            guard_violations: self.guard_violations,
            per_slot_ood_reasons: self.per_slot_ood_reasons,
            closest_exemplars: self.closest_exemplars,
            predicted_edge_cases: self.predicted_edge_cases,
            predicted_latent_bugs: self.predicted_latent_bugs,
            predicted_tech_debt_added: self.predicted_tech_debt_added,
            predicted_dead_code: self.predicted_dead_code,
            predicted_redundant_code: self.predicted_redundant_code,
            predicted_perf_regressions: self.predicted_perf_regressions,
            predicted_security_concerns: self.predicted_security_concerns,
            predicted_accuracy_degradations: self.predicted_accuracy_degradations,
            predicted_cost_regressions: self.predicted_cost_regressions,
            predicted_reasoning_class: self.predicted_reasoning_class,
            agent_claim_graph: self.agent_claim_graph,
            claim_reconciliation: self.claim_reconciliation,
            reality_impact: self.reality_impact,
            provenance: self.provenance,
            source_panel_sha: self.source_panel_sha,
            calibration_version: self.calibration_version,
            created_at_unix_ms: self.created_at_unix_ms,
            matched_fingerprint: self.matched_fingerprint,
            unknown_candidate_id: self.unknown_candidate_id,
            constellation_intelligence: None,
            slot_attributions: Vec::new(),
            label_context: PredictionLabelContext::default(),
        })
    }
}

pub fn decode_reality_prediction(bytes: &[u8]) -> Result<RealityPrediction, MejepaInferError> {
    match bincode_deserialize_strict::<RealityPrediction>(bytes) {
        Ok(prediction) => RealityPrediction::try_new(prediction),
        Err(current_err) => {
            if let Ok((legacy, consumed)) =
                bincode_deserialize_prefix::<RealityPredictionLegacyNoLabelContext>(bytes)
            {
                if consumed < bytes.len() {
                    if let Ok(label_context) = bincode_deserialize_strict::<
                        PredictionLabelContextLegacyV1,
                    >(&bytes[consumed..])
                    {
                        return legacy
                            .into_current_with_label_context(label_context.into_current());
                    }
                }
            }
            if let Ok(legacy) =
                bincode_deserialize_strict::<RealityPredictionLegacyNoLabelContext>(bytes)
            {
                return legacy.into_current();
            }
            if let Ok(legacy) =
                bincode_deserialize_strict::<RealityPredictionLegacyNoSlotAttributions>(bytes)
            {
                return legacy.into_current();
            }
            let legacy = bincode_deserialize_strict::<RealityPredictionLegacyNoConstellation>(
                bytes,
            )
            .map_err(|legacy_err| MejepaInferError::InvalidInput {
                field: "RealityPrediction.bincode".to_string(),
                detail: format!(
                    "failed to decode current schema ({current_err}) or legacy no-constellation schema ({legacy_err})"
                ),
            })?;
            legacy.into_current()
        }
    }
}

fn bincode_deserialize_strict<T>(bytes: &[u8]) -> Result<T, Box<bincode::ErrorKind>>
where
    T: for<'de> Deserialize<'de>,
{
    let mut cursor = Cursor::new(bytes);
    let value = bincode::deserialize_from(&mut cursor)?;
    let consumed = cursor.position() as usize;
    if consumed != bytes.len() {
        return Err(Box::new(bincode::ErrorKind::Custom(format!(
            "trailing bytes after strict decode: consumed {consumed} of {}",
            bytes.len()
        ))));
    }
    Ok(value)
}

fn bincode_deserialize_prefix<T>(bytes: &[u8]) -> Result<(T, usize), Box<bincode::ErrorKind>>
where
    T: for<'de> Deserialize<'de>,
{
    let mut cursor = Cursor::new(bytes);
    let value = bincode::deserialize_from(&mut cursor)?;
    Ok((value, cursor.position() as usize))
}

impl RealityPrediction {
    pub fn try_new(mut value: Self) -> Result<Self, MejepaInferError> {
        if value.slot_attributions.is_empty() {
            value.slot_attributions = derive_slot_attributions(&value)?;
        }
        value.validate()?;
        value.covered_chunks.shrink_to_fit();
        value.predicted_test_pass.shrink_to_fit();
        value.predicted_failure_modes.shrink_to_fit();
        value.predicted_failed_tests.shrink_to_fit();
        value.predicted_works.shrink_to_fit();
        value.predicted_uncovered_paths.shrink_to_fit();
        value.predicted_flaky_tests.shrink_to_fit();
        value.guard_violations.shrink_to_fit();
        value.per_slot_ood_reasons.shrink_to_fit();
        value.closest_exemplars.shrink_to_fit();
        value.predicted_edge_cases.shrink_to_fit();
        value.predicted_latent_bugs.shrink_to_fit();
        value.predicted_tech_debt_added.shrink_to_fit();
        value.predicted_dead_code.shrink_to_fit();
        value.predicted_redundant_code.shrink_to_fit();
        value.predicted_perf_regressions.shrink_to_fit();
        value.predicted_security_concerns.shrink_to_fit();
        value.predicted_accuracy_degradations.shrink_to_fit();
        value.predicted_cost_regressions.shrink_to_fit();
        value.claim_reconciliation.shrink_to_fit();
        value.slot_attributions.shrink_to_fit();
        value.label_context.accepted_label_ids.shrink_to_fit();
        value.label_context.failure_evidence_set_ids.shrink_to_fit();
        value.label_context.active_skill_ids.shrink_to_fit();
        Ok(value)
    }

    pub fn clear_q4_display_only_fields(&mut self) {
        self.predicted_edge_cases.clear();
        self.predicted_latent_bugs.clear();
        self.predicted_tech_debt_added.clear();
        self.predicted_dead_code.clear();
        self.predicted_redundant_code.clear();
        self.predicted_perf_regressions.clear();
        self.predicted_security_concerns.clear();
        self.predicted_accuracy_degradations.clear();
        self.predicted_cost_regressions.clear();
        self.predicted_reasoning_class = ReasoningClass::Mute;
        self.slot_attributions
            .retain(|item| item.source != SlotAttributionSource::Q4Head);
    }

    pub fn q4_display_only_field_count(&self) -> usize {
        self.predicted_edge_cases.len()
            + self.predicted_latent_bugs.len()
            + self.predicted_tech_debt_added.len()
            + self.predicted_dead_code.len()
            + self.predicted_redundant_code.len()
            + self.predicted_perf_regressions.len()
            + self.predicted_security_concerns.len()
            + self.predicted_accuracy_degradations.len()
            + self.predicted_cost_regressions.len()
            + usize::from(self.predicted_reasoning_class != ReasoningClass::Mute)
    }

    pub fn q4_display_only_fields_empty(&self) -> bool {
        self.q4_display_only_field_count() == 0
    }

    pub fn validate(&self) -> Result<(), MejepaInferError> {
        validate_non_empty_id("task_id", &self.task_id.0)?;
        if self.covered_chunks.len() > MAX_CHUNKS {
            return Err(MejepaInferError::DimMismatch {
                expected: MAX_CHUNKS,
                actual: self.covered_chunks.len(),
                context: "covered_chunks exceeds maximum".to_string(),
            });
        }
        for (idx, chunk) in self.covered_chunks.iter().enumerate() {
            chunk.validate(&format!("covered_chunks[{idx}]"))?;
        }
        self.confidence_interval.validate("confidence_interval")?;
        validate_probability("predicted_oracle_pass", self.predicted_oracle_pass)?;
        validate_probability("ood_score", self.ood_score)?;
        validate_probability("calibrated_confidence", self.calibrated_confidence)?;
        if self.predicted_test_pass.is_empty() {
            return Err(MejepaInferError::DimMismatch {
                expected: 1,
                actual: 0,
                context: "predicted_test_pass must be non-empty".to_string(),
            });
        }
        for (idx, p) in self.predicted_test_pass.iter().enumerate() {
            validate_probability(&format!("predicted_test_pass[{idx}]"), *p)?;
        }
        for (idx, v) in self.predicted_runtime_trace.iter().enumerate() {
            if !v.is_finite() {
                return Err(MejepaInferError::NanDetected {
                    nan_source: "predicted_runtime_trace".to_string(),
                    detail: format!("predicted_runtime_trace[{idx}] is {v}"),
                });
            }
        }
        for (key, value) in &self.granger_attestations {
            validate_non_empty_id("granger_attestations.key", key)?;
            validate_probability(&format!("granger_attestations[{key}]"), *value)?;
        }
        validate_items(
            "predicted_failure_modes",
            &self.predicted_failure_modes,
            PredictedFailureMode::validate,
        )?;
        validate_items(
            "predicted_failed_tests",
            &self.predicted_failed_tests,
            PredictedTestOutcome::validate,
        )?;
        validate_items(
            "predicted_works",
            &self.predicted_works,
            PredictedWorks::validate,
        )?;
        validate_items(
            "predicted_uncovered_paths",
            &self.predicted_uncovered_paths,
            UncoveredPath::validate,
        )?;
        validate_items(
            "predicted_flaky_tests",
            &self.predicted_flaky_tests,
            FlakyTestCandidate::validate,
        )?;
        validate_items(
            "guard_violations",
            &self.guard_violations,
            GuardViolation::validate,
        )?;
        if self.per_slot_ood_reasons.len() > MAX_EMBEDDERS_PER_PANEL {
            return Err(MejepaInferError::DimMismatch {
                expected: MAX_EMBEDDERS_PER_PANEL,
                actual: self.per_slot_ood_reasons.len(),
                context: "per_slot_ood_reasons exceeds maximum".to_string(),
            });
        }
        validate_items(
            "per_slot_ood_reasons",
            &self.per_slot_ood_reasons,
            PerSlotOodReason::validate,
        )?;
        validate_items(
            "closest_exemplars",
            &self.closest_exemplars,
            ExemplarMatch::validate,
        )?;
        validate_items(
            "predicted_edge_cases",
            &self.predicted_edge_cases,
            PredictedEdgeCase::validate,
        )?;
        validate_items(
            "predicted_latent_bugs",
            &self.predicted_latent_bugs,
            PredictedLatentBug::validate,
        )?;
        validate_items(
            "predicted_tech_debt_added",
            &self.predicted_tech_debt_added,
            PredictedTechDebt::validate,
        )?;
        validate_items(
            "predicted_dead_code",
            &self.predicted_dead_code,
            PredictedDeadCode::validate,
        )?;
        validate_items(
            "predicted_redundant_code",
            &self.predicted_redundant_code,
            PredictedRedundancy::validate,
        )?;
        validate_items(
            "predicted_perf_regressions",
            &self.predicted_perf_regressions,
            PredictedPerfRegression::validate,
        )?;
        validate_items(
            "predicted_security_concerns",
            &self.predicted_security_concerns,
            PredictedSecurityConcern::validate,
        )?;
        validate_items(
            "predicted_accuracy_degradations",
            &self.predicted_accuracy_degradations,
            PredictedAccuracyDegradation::validate,
        )?;
        validate_items(
            "predicted_cost_regressions",
            &self.predicted_cost_regressions,
            PredictedCostRegression::validate,
        )?;
        self.agent_claim_graph.validate()?;
        validate_items(
            "claim_reconciliation",
            &self.claim_reconciliation,
            ClaimReconciliation::validate,
        )?;
        if let Some(impact) = &self.reality_impact {
            impact.validate()?;
        }
        self.provenance.validate()?;
        self.outcome_set.validate()?;
        if let Some(matched) = &self.matched_fingerprint {
            matched.validate()?;
        }
        if let Some(evidence) = &self.constellation_intelligence {
            evidence.validate()?;
        }
        self.label_context.validate()?;
        validate_slot_attribution_collection(
            "slot_attributions",
            &self.slot_attributions,
            Some(self.verdict),
        )?;
        if self.unknown_candidate_id.is_some() && self.verdict != Verdict::OutOfDistribution {
            return Err(MejepaInferError::InvalidInput {
                field: "unknown_candidate_id".to_string(),
                detail: "unknown_candidate_id is only valid for OutOfDistribution verdicts"
                    .to_string(),
            });
        }
        if let Some(candidate_id) = &self.unknown_candidate_id {
            if candidate_id.iter().all(|byte| *byte == 0) {
                return Err(MejepaInferError::InvalidInput {
                    field: "unknown_candidate_id".to_string(),
                    detail: "unknown_candidate_id must be non-zero".to_string(),
                });
            }
        }
        if self.matched_fingerprint.is_some()
            && matches!(
                self.verdict,
                Verdict::OutOfDistribution | Verdict::GuardRejected
            )
        {
            return Err(MejepaInferError::InvalidInput {
                field: "matched_fingerprint".to_string(),
                detail: "matched_fingerprint must be None for OutOfDistribution or GuardRejected"
                    .to_string(),
            });
        }
        if self.calibration_version.is_empty() {
            return Err(MejepaInferError::InvalidInput {
                field: "calibration_version".to_string(),
                detail: "calibration_version must be non-empty".to_string(),
            });
        }
        reject_control_chars("calibration_version", &self.calibration_version)?;
        Ok(())
    }
}

fn validate_slot_attribution_collection(
    field: &str,
    attributions: &[SlotAttributionEvidence],
    verdict: Option<Verdict>,
) -> Result<(), MejepaInferError> {
    if attributions.len() > MAX_SLOT_ATTRIBUTIONS {
        return Err(MejepaInferError::DimMismatch {
            expected: MAX_SLOT_ATTRIBUTIONS,
            actual: attributions.len(),
            context: format!("{field} exceeds maximum"),
        });
    }
    validate_items(field, attributions, SlotAttributionEvidence::validate)?;
    let mut seen = BTreeSet::new();
    for item in attributions {
        let key = format!(
            "{}|{:?}|{:?}|{}|{}",
            item.slot_id,
            item.source,
            item.polarity,
            item.chunk
                .as_ref()
                .map(|chunk| chunk.0.as_str())
                .unwrap_or(""),
            item.reason
        );
        if !seen.insert(key) {
            return Err(MejepaInferError::InvalidInput {
                field: field.to_string(),
                detail: "duplicate slot attribution evidence".to_string(),
            });
        }
    }
    if let Some(verdict) = verdict {
        if verdict != Verdict::Abstain && attributions.is_empty() {
            return Err(MejepaInferError::InvalidInput {
                field: field.to_string(),
                detail: "non-abstain verdicts must carry at least one slot attribution".to_string(),
            });
        }
        if verdict_requires_rejection_evidence(verdict)
            && !attributions
                .iter()
                .any(|item| item.polarity.is_rejection_evidence())
        {
            return Err(MejepaInferError::InvalidInput {
                field: field.to_string(),
                detail: "rejection verdicts must name a violating, missing, or stale slot"
                    .to_string(),
            });
        }
    }
    Ok(())
}

fn verdict_requires_rejection_evidence(verdict: Verdict) -> bool {
    matches!(
        verdict,
        Verdict::Fail | Verdict::OutOfDistribution | Verdict::Abstain | Verdict::GuardRejected
    )
}

fn derive_slot_attributions(
    prediction: &RealityPrediction,
) -> Result<Vec<SlotAttributionEvidence>, MejepaInferError> {
    let mut acc = SlotAttributionAccumulator::default();
    let default_chunk = prediction.covered_chunks.first().cloned();

    for work in &prediction.predicted_works {
        for embedder in &work.supporting_embedders {
            acc.push(slot_attribution(
                slot_id_for_embedder(embedder),
                Some(embedder.clone()),
                Some(work.chunk.clone()),
                SlotAttributionPolarity::Supporting,
                SlotAttributionSource::PredictedWorks,
                work.evidence_strength.max(work.confidence),
                None,
                None,
                "predicted work support",
                None,
                None,
                None,
                None,
                None,
                &work.claim,
            ));
        }
    }

    for mode in &prediction.predicted_failure_modes {
        if mode.contributing_embedders.is_empty() {
            acc.push(slot_attribution(
                "q3_failure_mode_head",
                None,
                Some(mode.chunk.clone()),
                SlotAttributionPolarity::Violating,
                SlotAttributionSource::FailureMode,
                mode.confidence,
                None,
                None,
                "failure mode head violation",
                None,
                None,
                None,
                Some("q3_failure_mode".to_string()),
                None,
                &mode.explanation,
            ));
        }
        for embedder in &mode.contributing_embedders {
            acc.push(slot_attribution(
                slot_id_for_embedder(embedder),
                Some(embedder.clone()),
                Some(mode.chunk.clone()),
                SlotAttributionPolarity::Violating,
                SlotAttributionSource::FailureMode,
                mode.confidence,
                None,
                None,
                "failure mode contributing embedder",
                None,
                None,
                None,
                Some("q3_failure_mode".to_string()),
                None,
                &mode.explanation,
            ));
        }
    }

    for guard in &prediction.guard_violations {
        acc.push(slot_attribution(
            slot_id_for_embedder(&guard.embedder),
            Some(guard.embedder.clone()),
            Some(guard.chunk.clone()),
            SlotAttributionPolarity::Violating,
            SlotAttributionSource::GuardViolation,
            guard.deficit.clamp(0.0, 1.0),
            Some(guard.threshold_tau_m),
            Some(guard.deficit),
            "constellation guard violation",
            None,
            None,
            None,
            None,
            None,
            format!(
                "centroid={} cosine={} threshold={}",
                guard.centroid_id, guard.cosine, guard.threshold_tau_m
            ),
        ));
    }

    for reason in &prediction.per_slot_ood_reasons {
        acc.push(slot_attribution(
            slot_id_for_embedder(&reason.embedder),
            Some(reason.embedder.clone()),
            reason.chunk.clone(),
            polarity_for_ood_reason(reason.reason),
            SlotAttributionSource::PerSlotOod,
            reason.margin.clamp(0.0, 1.0),
            Some(reason.threshold),
            Some(reason.margin),
            "per-slot OOD guard",
            None,
            None,
            None,
            None,
            None,
            &reason.evidence,
        ));
    }

    if let Some(evidence) = &prediction.constellation_intelligence {
        for pair in &evidence.slot_pair_evidence {
            let pair_score = pair
                .contradiction_score
                .max(pair.novelty_score)
                .max(pair.blind_spot_z_score)
                .max(pair.relationship_score)
                .clamp(0.0, 1.0);
            for (slot, other) in [
                (&pair.left_slot_id, &pair.right_slot_id),
                (&pair.right_slot_id, &pair.left_slot_id),
            ] {
                acc.push(slot_attribution(
                    slot.clone(),
                    None,
                    default_chunk.clone(),
                    SlotAttributionPolarity::Relationship,
                    SlotAttributionSource::ConstellationPair,
                    pair_score,
                    None,
                    pair.oracle_failure_lift_over_single_slot,
                    "cross-slot relationship evidence",
                    Some(other.clone()),
                    evidence.relationship_pattern_id.clone(),
                    None,
                    None,
                    None,
                    &pair.source,
                ));
            }
        }
    }

    if let Some(matched) = &prediction.matched_fingerprint {
        acc.push(slot_attribution(
            "failure_fingerprint_catalog",
            None,
            default_chunk.clone(),
            SlotAttributionPolarity::Supporting,
            SlotAttributionSource::FailureFingerprint,
            matched.mean_cosine.clamp(0.0, 1.0),
            None,
            Some(matched.min_margin.max(0.0)),
            "matched failure-shape fingerprint",
            None,
            Some(hex::encode(matched.fingerprint_id)),
            None,
            None,
            None,
            &matched.name,
        ));
    }

    if let Some(candidate_id) = prediction.unknown_candidate_id {
        acc.push(slot_attribution(
            "active_learning_queue",
            None,
            default_chunk.clone(),
            SlotAttributionPolarity::Missing,
            SlotAttributionSource::ActiveLearningCandidate,
            prediction.ood_score,
            None,
            None,
            "unknown OOD candidate queued for labeling",
            None,
            None,
            Some(candidate_id),
            None,
            None,
            "no known fingerprint or calibrated slot support matched this observation",
        ));
    }

    derive_q4_attributions(prediction, &mut acc, default_chunk.clone());
    if let Some(impact) = &prediction.reality_impact {
        acc.push(slot_attribution(
            "q5_reality_impact_head",
            None,
            default_chunk.clone(),
            SlotAttributionPolarity::Q5Impact,
            SlotAttributionSource::Q5Replay,
            q5_impact_score(impact.prediction_correctness),
            None,
            None,
            "reality-impact replay evidence",
            None,
            None,
            None,
            None,
            Some(format!("{:?}", impact.prediction_correctness).to_ascii_lowercase()),
            "Q5 replay compared predicted and observed durable reality shifts",
        ));
    }

    for (key, score) in &prediction.granger_attestations {
        acc.push(slot_attribution(
            granger_slot_id(key),
            None,
            default_chunk.clone(),
            SlotAttributionPolarity::Supporting,
            SlotAttributionSource::GrangerAttestation,
            *score,
            None,
            None,
            "DDA/Granger attestation",
            None,
            None,
            None,
            None,
            None,
            key,
        ));
    }

    if acc.items.is_empty() {
        acc.push(slot_attribution(
            "verdict_head",
            None,
            default_chunk.clone(),
            SlotAttributionPolarity::Supporting,
            SlotAttributionSource::VerdictHead,
            prediction
                .predicted_oracle_pass
                .max(prediction.calibrated_confidence)
                .clamp(0.0, 1.0),
            None,
            None,
            "verdict head support",
            None,
            None,
            None,
            Some("q2_verdict".to_string()),
            None,
            "fallback attribution from calibrated Q2 verdict score",
        ));
    }
    if verdict_requires_rejection_evidence(prediction.verdict)
        && !acc
            .items
            .iter()
            .any(|item| item.polarity.is_rejection_evidence())
    {
        acc.push(slot_attribution(
            "verdict_head",
            None,
            default_chunk,
            SlotAttributionPolarity::Violating,
            SlotAttributionSource::VerdictHead,
            (1.0 - prediction.predicted_oracle_pass).clamp(0.0, 1.0),
            None,
            None,
            "rejection verdict evidence",
            None,
            None,
            None,
            Some("q2_verdict".to_string()),
            None,
            "fallback rejection attribution from calibrated Q2 verdict score",
        ));
    }
    Ok(acc.items)
}

fn derive_q4_attributions(
    prediction: &RealityPrediction,
    acc: &mut SlotAttributionAccumulator,
    default_chunk: Option<ChunkId>,
) {
    for item in &prediction.predicted_edge_cases {
        acc.push(q4_attribution(
            "q4_edge_case_head",
            "edge_case",
            item.chunk.clone(),
            item.confidence,
            &item.triggering_input_description,
        ));
    }
    for item in &prediction.predicted_latent_bugs {
        acc.push(q4_attribution(
            "q4_latent_bug_head",
            "latent_bug",
            item.chunk.clone(),
            item.confidence,
            &item.explanation,
        ));
    }
    for item in &prediction.predicted_tech_debt_added {
        acc.push(q4_attribution(
            "q4_tech_debt_head",
            "tech_debt",
            item.chunk.clone(),
            severity_score(item.severity),
            &item.explanation,
        ));
    }
    for item in &prediction.predicted_dead_code {
        acc.push(q4_attribution(
            "q4_dead_code_head",
            "dead_code",
            item.chunk.clone(),
            0.6,
            &item.reason,
        ));
    }
    for item in &prediction.predicted_redundant_code {
        acc.push(q4_attribution(
            "q4_redundancy_head",
            "redundancy",
            item.chunk.clone(),
            item.similarity,
            &item.explanation,
        ));
    }
    for item in &prediction.predicted_perf_regressions {
        acc.push(q4_attribution(
            "q4_perf_head",
            "perf",
            item.chunk.clone(),
            item.confidence,
            &item.explanation,
        ));
    }
    for item in &prediction.predicted_security_concerns {
        acc.push(q4_attribution(
            "q4_security_head",
            "security",
            item.chunk.clone(),
            item.cvss_estimate.map(|cvss| cvss / 10.0).unwrap_or(0.7),
            &item.explanation,
        ));
    }
    for item in &prediction.predicted_accuracy_degradations {
        acc.push(q4_attribution(
            "q4_accuracy_head",
            "accuracy",
            item.chunk.clone(),
            item.confidence,
            &item.explanation,
        ));
    }
    for item in &prediction.predicted_cost_regressions {
        acc.push(q4_attribution(
            "q4_cost_head",
            "cost",
            item.chunk.clone(),
            scaled_delta(item.predicted_delta as f32),
            &item.explanation,
        ));
    }
    if prediction.predicted_reasoning_class != ReasoningClass::Mute {
        acc.push(slot_attribution(
            "q4_reasoning_head",
            None,
            default_chunk,
            SlotAttributionPolarity::Q4Concern,
            SlotAttributionSource::Q4Head,
            0.5,
            None,
            None,
            "reasoning-class head evidence",
            None,
            None,
            None,
            Some("reasoning".to_string()),
            None,
            format!("{:?}", prediction.predicted_reasoning_class),
        ));
    }
}

#[derive(Default)]
struct SlotAttributionAccumulator {
    items: Vec<SlotAttributionEvidence>,
    seen: BTreeSet<String>,
}

impl SlotAttributionAccumulator {
    fn push(&mut self, item: SlotAttributionEvidence) {
        if self.items.len() >= MAX_SLOT_ATTRIBUTIONS {
            return;
        }
        let key = format!(
            "{}|{:?}|{:?}|{}|{}",
            item.slot_id,
            item.source,
            item.polarity,
            item.chunk
                .as_ref()
                .map(|chunk| chunk.0.as_str())
                .unwrap_or(""),
            item.reason
        );
        if self.seen.insert(key) {
            self.items.push(item);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn slot_attribution(
    slot_id: impl Into<String>,
    embedder: Option<EmbedderId>,
    chunk: Option<ChunkId>,
    polarity: SlotAttributionPolarity,
    source: SlotAttributionSource,
    score: f32,
    threshold: Option<f32>,
    margin: Option<f32>,
    reason: impl Into<String>,
    relationship_slot_id: Option<String>,
    related_fingerprint_id: Option<String>,
    active_learning_candidate_id: Option<[u8; 16]>,
    q_head: Option<String>,
    impact_kind: Option<String>,
    evidence: impl Into<String>,
) -> SlotAttributionEvidence {
    SlotAttributionEvidence {
        schema_version: SLOT_ATTRIBUTION_SCHEMA_VERSION,
        slot_id: slot_id.into(),
        embedder,
        chunk,
        polarity,
        source,
        score: score.clamp(0.0, 1.0),
        threshold,
        margin,
        reason: reason.into(),
        relationship_slot_id,
        related_fingerprint_id,
        active_learning_candidate_id,
        q_head,
        impact_kind,
        evidence: evidence.into(),
    }
}

fn q4_attribution(
    slot_id: &'static str,
    q_head: &'static str,
    chunk: ChunkId,
    score: f32,
    evidence: &str,
) -> SlotAttributionEvidence {
    slot_attribution(
        slot_id,
        None,
        Some(chunk),
        SlotAttributionPolarity::Q4Concern,
        SlotAttributionSource::Q4Head,
        score,
        None,
        None,
        "Q4 consequence head evidence",
        None,
        None,
        None,
        Some(q_head.to_string()),
        None,
        evidence,
    )
}

fn slot_id_for_embedder(embedder: &EmbedderId) -> String {
    if embedder.0.trim().is_empty() {
        "unknown_embedder".to_string()
    } else {
        embedder.0.clone()
    }
}

fn polarity_for_ood_reason(reason: PerSlotOodReasonKind) -> SlotAttributionPolarity {
    match reason {
        PerSlotOodReasonKind::MissingSlotObservation => SlotAttributionPolarity::Missing,
        PerSlotOodReasonKind::StaleCalibration => SlotAttributionPolarity::Stale,
        _ => SlotAttributionPolarity::Violating,
    }
}

fn granger_slot_id(key: &str) -> String {
    let mut out = String::from("granger_");
    for ch in key.chars().take(64) {
        out.push(if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
            ch
        } else {
            '_'
        });
    }
    out
}

fn severity_score(severity: Severity) -> f32 {
    match severity {
        Severity::Critical => 1.0,
        Severity::High => 0.8,
        Severity::Medium => 0.6,
        Severity::Low => 0.4,
        Severity::Info => 0.2,
    }
}

fn q5_impact_score(correctness: PredictionCorrectness) -> f32 {
    match correctness {
        PredictionCorrectness::Aligned => 1.0,
        PredictionCorrectness::UnderPredicted | PredictionCorrectness::OverPredicted => 0.7,
        PredictionCorrectness::DivergentInClass => 0.8,
        PredictionCorrectness::Surprise => 0.95,
    }
}

fn scaled_delta(value: f32) -> f32 {
    (value.abs() / (value.abs() + 1.0)).clamp(0.0, 1.0)
}

#[derive(Debug, Clone)]
pub struct RealityPredictionBuilder {
    prediction_id: [u8; 16],
    witness_hash: WitnessHash,
    task_id: TaskId,
    session_id: [u8; 16],
    language: Language,
    covered_chunks: Vec<ChunkId>,
    verdict: Verdict,
    confidence_interval: ConformalInterval,
    predicted_oracle_pass: f32,
    predicted_test_pass: Vec<f32>,
    predicted_runtime_trace: [f32; 32],
    ood_score: f32,
    outcome_set: ConformalSet,
    calibrated_confidence: f32,
    degraded_status: bool,
    granger_attestations: BTreeMap<String, f32>,
    predicted_failure_modes: Vec<PredictedFailureMode>,
    predicted_failed_tests: Vec<PredictedTestOutcome>,
    predicted_works: Vec<PredictedWorks>,
    predicted_uncovered_paths: Vec<UncoveredPath>,
    predicted_flaky_tests: Vec<FlakyTestCandidate>,
    guard_violations: Vec<GuardViolation>,
    per_slot_ood_reasons: Vec<PerSlotOodReason>,
    closest_exemplars: Vec<ExemplarMatch>,
    predicted_edge_cases: Vec<PredictedEdgeCase>,
    predicted_latent_bugs: Vec<PredictedLatentBug>,
    predicted_tech_debt_added: Vec<PredictedTechDebt>,
    predicted_dead_code: Vec<PredictedDeadCode>,
    predicted_redundant_code: Vec<PredictedRedundancy>,
    predicted_perf_regressions: Vec<PredictedPerfRegression>,
    predicted_security_concerns: Vec<PredictedSecurityConcern>,
    predicted_accuracy_degradations: Vec<PredictedAccuracyDegradation>,
    predicted_cost_regressions: Vec<PredictedCostRegression>,
    predicted_reasoning_class: ReasoningClass,
    agent_claim_graph: AgentClaimGraph,
    claim_reconciliation: Vec<ClaimReconciliation>,
    reality_impact: Option<RealityImpact>,
    provenance: PredictionProvenance,
    source_panel_sha: [u8; 32],
    calibration_version: String,
    created_at_unix_ms: i64,
    // TASK-FP-010 (#319) — see RealityPrediction docs for verdict→field semantics.
    matched_fingerprint: Option<MatchedFingerprintEvidence>,
    unknown_candidate_id: Option<[u8; 16]>,
    constellation_intelligence: Option<ConstellationIntelligenceEvidence>,
    slot_attributions: Vec<SlotAttributionEvidence>,
    label_context: PredictionLabelContext,
}

impl RealityPredictionBuilder {
    pub fn from_parts(
        task_id: TaskId,
        session_id: [u8; 16],
        language: Language,
        outcome_set: ConformalSet,
    ) -> Self {
        Self {
            prediction_id: [0u8; 16],
            witness_hash: WitnessHash([0u8; 32]),
            task_id,
            session_id,
            language,
            covered_chunks: Vec::new(),
            verdict: Verdict::Abstain,
            confidence_interval: ConformalInterval::default(),
            predicted_oracle_pass: 0.0,
            predicted_test_pass: Vec::new(),
            predicted_runtime_trace: [0.0; 32],
            ood_score: 0.0,
            outcome_set,
            calibrated_confidence: 0.0,
            degraded_status: false,
            granger_attestations: BTreeMap::new(),
            predicted_failure_modes: Vec::new(),
            predicted_failed_tests: Vec::new(),
            predicted_works: Vec::new(),
            predicted_uncovered_paths: Vec::new(),
            predicted_flaky_tests: Vec::new(),
            guard_violations: Vec::new(),
            per_slot_ood_reasons: Vec::new(),
            closest_exemplars: Vec::new(),
            predicted_edge_cases: Vec::new(),
            predicted_latent_bugs: Vec::new(),
            predicted_tech_debt_added: Vec::new(),
            predicted_dead_code: Vec::new(),
            predicted_redundant_code: Vec::new(),
            predicted_perf_regressions: Vec::new(),
            predicted_security_concerns: Vec::new(),
            predicted_accuracy_degradations: Vec::new(),
            predicted_cost_regressions: Vec::new(),
            predicted_reasoning_class: ReasoningClass::Mute,
            agent_claim_graph: AgentClaimGraph::default(),
            claim_reconciliation: Vec::new(),
            reality_impact: None,
            provenance: PredictionProvenance::default(),
            source_panel_sha: [0u8; 32],
            calibration_version: String::new(),
            created_at_unix_ms: chrono::Utc::now().timestamp_millis(),
            matched_fingerprint: None,
            unknown_candidate_id: None,
            constellation_intelligence: None,
            slot_attributions: Vec::new(),
            label_context: PredictionLabelContext::default(),
        }
    }

    pub fn prediction_id(mut self, prediction_id: [u8; 16]) -> Self {
        self.prediction_id = prediction_id;
        self
    }

    pub fn witness_hash(mut self, witness_hash: WitnessHash) -> Self {
        self.witness_hash = witness_hash;
        self
    }

    pub fn covered_chunks(mut self, value: Vec<ChunkId>) -> Self {
        self.covered_chunks = value;
        self
    }

    pub fn verdict(mut self, value: Verdict) -> Self {
        self.verdict = value;
        self
    }

    pub fn confidence_interval(mut self, value: ConformalInterval) -> Self {
        self.confidence_interval = value;
        self
    }

    pub fn predicted_oracle_pass(mut self, value: f32) -> Self {
        self.predicted_oracle_pass = value;
        self
    }

    pub fn predicted_test_pass(mut self, value: Vec<f32>) -> Self {
        self.predicted_test_pass = value;
        self
    }

    pub fn predicted_runtime_trace(mut self, value: [f32; 32]) -> Self {
        self.predicted_runtime_trace = value;
        self
    }

    pub fn ood_score(mut self, value: f32) -> Self {
        self.ood_score = value;
        self
    }

    pub fn calibrated_confidence(mut self, value: f32) -> Self {
        self.calibrated_confidence = value;
        self
    }

    pub fn degraded_status(mut self, value: bool) -> Self {
        self.degraded_status = value;
        self
    }

    pub fn granger_attestations(mut self, value: BTreeMap<String, f32>) -> Self {
        self.granger_attestations = value;
        self
    }

    pub fn predicted_failure_modes(mut self, value: Vec<PredictedFailureMode>) -> Self {
        self.predicted_failure_modes = value;
        self
    }

    pub fn predicted_failed_tests(mut self, value: Vec<PredictedTestOutcome>) -> Self {
        self.predicted_failed_tests = value;
        self
    }

    pub fn predicted_works(mut self, value: Vec<PredictedWorks>) -> Self {
        self.predicted_works = value;
        self
    }

    pub fn predicted_uncovered_paths(mut self, value: Vec<UncoveredPath>) -> Self {
        self.predicted_uncovered_paths = value;
        self
    }

    pub fn predicted_flaky_tests(mut self, value: Vec<FlakyTestCandidate>) -> Self {
        self.predicted_flaky_tests = value;
        self
    }

    pub fn guard_violations(mut self, value: Vec<GuardViolation>) -> Self {
        self.guard_violations = value;
        self
    }

    pub fn per_slot_ood_reasons(mut self, value: Vec<PerSlotOodReason>) -> Self {
        self.per_slot_ood_reasons = value;
        self
    }

    pub fn predicted_edge_cases(mut self, value: Vec<PredictedEdgeCase>) -> Self {
        self.predicted_edge_cases = value;
        self
    }

    pub fn predicted_latent_bugs(mut self, value: Vec<PredictedLatentBug>) -> Self {
        self.predicted_latent_bugs = value;
        self
    }

    pub fn predicted_security_concerns(mut self, value: Vec<PredictedSecurityConcern>) -> Self {
        self.predicted_security_concerns = value;
        self
    }

    pub fn predicted_reasoning_class(mut self, value: ReasoningClass) -> Self {
        self.predicted_reasoning_class = value;
        self
    }

    pub fn agent_claim_graph(mut self, value: AgentClaimGraph) -> Self {
        self.agent_claim_graph = value;
        self
    }

    pub fn claim_reconciliation(mut self, value: Vec<ClaimReconciliation>) -> Self {
        self.claim_reconciliation = value;
        self
    }

    pub fn reality_impact(mut self, value: Option<RealityImpact>) -> Self {
        self.reality_impact = value;
        self
    }

    pub fn phase_b_surfaces(mut self, value: PhaseBPredictionSurfaces) -> Self {
        self.predicted_failure_modes = value.predicted_failure_modes;
        self.predicted_failed_tests = value.predicted_failed_tests;
        self.predicted_works = value.predicted_works;
        self.predicted_uncovered_paths = value.predicted_uncovered_paths;
        self.predicted_flaky_tests = value.predicted_flaky_tests;
        self.guard_violations = value.guard_violations;
        self.closest_exemplars = value.closest_exemplars;
        self.predicted_edge_cases = value.predicted_edge_cases;
        self.predicted_latent_bugs = value.predicted_latent_bugs;
        self.predicted_tech_debt_added = value.predicted_tech_debt_added;
        self.predicted_dead_code = value.predicted_dead_code;
        self.predicted_redundant_code = value.predicted_redundant_code;
        self.predicted_perf_regressions = value.predicted_perf_regressions;
        self.predicted_security_concerns = value.predicted_security_concerns;
        self.predicted_accuracy_degradations = value.predicted_accuracy_degradations;
        self.predicted_cost_regressions = value.predicted_cost_regressions;
        self.predicted_reasoning_class = value.predicted_reasoning_class;
        self
    }

    pub fn provenance(mut self, value: PredictionProvenance) -> Self {
        self.provenance = value;
        self
    }

    pub fn source_panel_sha(mut self, value: [u8; 32]) -> Self {
        self.source_panel_sha = value;
        self
    }

    pub fn calibration_version(mut self, value: impl Into<String>) -> Self {
        self.calibration_version = value.into();
        self
    }

    pub fn created_at_unix_ms(mut self, value: i64) -> Self {
        self.created_at_unix_ms = value;
        self
    }

    pub fn build(self) -> Result<RealityPrediction, MejepaInferError> {
        let mut prediction = RealityPrediction {
            prediction_id: self.prediction_id,
            witness_hash: self.witness_hash,
            task_id: self.task_id,
            session_id: self.session_id,
            language: self.language,
            covered_chunks: self.covered_chunks,
            verdict: self.verdict,
            confidence_interval: self.confidence_interval,
            predicted_oracle_pass: self.predicted_oracle_pass,
            predicted_test_pass: self.predicted_test_pass,
            predicted_runtime_trace: self.predicted_runtime_trace,
            ood_score: self.ood_score,
            outcome_set: self.outcome_set,
            calibrated_confidence: self.calibrated_confidence,
            degraded_status: self.degraded_status,
            granger_attestations: self.granger_attestations,
            predicted_failure_modes: self.predicted_failure_modes,
            predicted_failed_tests: self.predicted_failed_tests,
            predicted_works: self.predicted_works,
            predicted_uncovered_paths: self.predicted_uncovered_paths,
            predicted_flaky_tests: self.predicted_flaky_tests,
            guard_violations: self.guard_violations,
            per_slot_ood_reasons: self.per_slot_ood_reasons,
            closest_exemplars: self.closest_exemplars,
            predicted_edge_cases: self.predicted_edge_cases,
            predicted_latent_bugs: self.predicted_latent_bugs,
            predicted_tech_debt_added: self.predicted_tech_debt_added,
            predicted_dead_code: self.predicted_dead_code,
            predicted_redundant_code: self.predicted_redundant_code,
            predicted_perf_regressions: self.predicted_perf_regressions,
            predicted_security_concerns: self.predicted_security_concerns,
            predicted_accuracy_degradations: self.predicted_accuracy_degradations,
            predicted_cost_regressions: self.predicted_cost_regressions,
            predicted_reasoning_class: self.predicted_reasoning_class,
            agent_claim_graph: self.agent_claim_graph,
            claim_reconciliation: self.claim_reconciliation,
            reality_impact: self.reality_impact,
            provenance: self.provenance,
            source_panel_sha: self.source_panel_sha,
            calibration_version: self.calibration_version,
            created_at_unix_ms: self.created_at_unix_ms,
            // TASK-FP-010 (#319) — builder defaults to None; live callers
            // populate via `.matched_fingerprint(...)` and
            // `.unknown_candidate_id(...)` setters below.
            matched_fingerprint: self.matched_fingerprint,
            unknown_candidate_id: self.unknown_candidate_id,
            constellation_intelligence: self.constellation_intelligence,
            slot_attributions: self.slot_attributions,
            label_context: self.label_context,
        };
        if crate::q4_trust_gate::Q4_DOCTRINE_FREEZE_ACTIVE {
            prediction.clear_q4_display_only_fields();
        }
        RealityPrediction::try_new(prediction)
    }

    pub fn matched_fingerprint(mut self, value: Option<MatchedFingerprintEvidence>) -> Self {
        self.matched_fingerprint = value;
        self
    }

    pub fn unknown_candidate_id(mut self, value: Option<[u8; 16]>) -> Self {
        self.unknown_candidate_id = value;
        self
    }

    pub fn constellation_intelligence(
        mut self,
        value: Option<ConstellationIntelligenceEvidence>,
    ) -> Self {
        self.constellation_intelligence = value;
        self
    }

    pub fn slot_attributions(mut self, value: Vec<SlotAttributionEvidence>) -> Self {
        self.slot_attributions = value;
        self
    }

    pub fn label_context(mut self, value: PredictionLabelContext) -> Self {
        self.label_context = value;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "verdict")]
pub enum VerifyVerdict {
    Approve {
        reality_prediction: RealityPrediction,
        gates_passed: u8,
    },
    EscalateToHuman {
        reality_prediction: Option<RealityPrediction>,
        failed_gate: FailedGate,
        gates_passed: u8,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum FailedGate {
    OutOfDistribution {
        ood_score: f32,
        threshold: f32,
    },
    OodGateRejected {
        reason: String,
        ood_score: f32,
        threshold: Option<f32>,
    },
    LowConfidence {
        outcome_set_len: usize,
        max_len: usize,
    },
    PredictedFailure {
        predicted_oracle_pass: f32,
        threshold: f32,
    },
    GrangerRejection {
        rejected: BTreeMap<String, f32>,
        threshold: f32,
    },
    WitnessChainBroken {
        reason: String,
    },
    SourceShaDrift {
        path: PathBuf,
    },
    ConstellationGuardRejected {
        reason: String,
    },
    SafetyConstraintViolation {
        violation_count: usize,
        total_cost: f32,
        cost_ceiling: f32,
        reason: String,
    },
    PredictionPaused {
        paused_until_unix_ms: i64,
        reason: String,
    },
    PredictionParked {
        attempt_count: u32,
        park_until_unix_ms: i64,
        reason: String,
    },
    HeadFailure {
        head: String,
        code: String,
        detail: String,
    },
    MultiHeadContradiction {
        cell_id: String,
        reason: String,
        oracle_pass_confidence: f32,
        tau_oracle: Option<f32>,
        high_severity_failure_count: u32,
        tau_failure_count: Option<u32>,
        security_concern_count: u32,
    },
    ColdCell {
        cell_id: String,
        n_supporting: Option<u32>,
        threshold: u32,
        reason: String,
    },
    WideInterval {
        interval_width: f32,
        threshold: f32,
        reason: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiffHunk {
    pub path: PathBuf,
    pub pre_sha: [u8; 32],
    pub post_sha: [u8; 32],
    pub before: String,
    pub after: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AstDiff {
    pub hunks: Vec<DiffHunk>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PatchBundle {
    pub ast_diff: AstDiff,
    pub witness_chain_segment: Vec<u8>,
    pub commit_message: String,
    pub patch_sha: [u8; 32],
}

impl PatchBundle {
    pub fn try_new(
        ast_diff: AstDiff,
        witness_chain_segment: Vec<u8>,
        commit_message: String,
        patch_sha: [u8; 32],
    ) -> Result<Self, MejepaInferError> {
        let value = Self {
            ast_diff,
            witness_chain_segment,
            commit_message,
            patch_sha,
        };
        value.validate()?;
        Ok(value)
    }

    pub fn validate(&self) -> Result<(), MejepaInferError> {
        if self.ast_diff.hunks.is_empty() {
            return Err(MejepaInferError::InvalidInput {
                field: "ast_diff.hunks".to_string(),
                detail: "patch bundle must contain at least one hunk".to_string(),
            });
        }
        if self.ast_diff.hunks.len() > MAX_PATCH_HUNKS {
            return Err(MejepaInferError::DimMismatch {
                expected: MAX_PATCH_HUNKS,
                actual: self.ast_diff.hunks.len(),
                context: "ast_diff.hunks exceeds maximum".to_string(),
            });
        }
        for (idx, hunk) in self.ast_diff.hunks.iter().enumerate() {
            validate_relative_path(&format!("ast_diff.hunks[{idx}].path"), &hunk.path)?;
            validate_bounded_text(
                &format!("ast_diff.hunks[{idx}].before"),
                &hunk.before,
                MAX_HUNK_TEXT_BYTES,
            )?;
            validate_bounded_text(
                &format!("ast_diff.hunks[{idx}].after"),
                &hunk.after,
                MAX_HUNK_TEXT_BYTES,
            )?;
        }
        if self.witness_chain_segment.len() > MAX_WITNESS_SEGMENT_BYTES {
            return Err(MejepaInferError::DimMismatch {
                expected: MAX_WITNESS_SEGMENT_BYTES,
                actual: self.witness_chain_segment.len(),
                context: "witness_chain_segment exceeds maximum bytes".to_string(),
            });
        }
        if !self
            .witness_chain_segment
            .len()
            .is_multiple_of(context_graph_witness::WITNESS_ENTRY_SIZE)
        {
            return Err(MejepaInferError::DimMismatch {
                expected: context_graph_witness::WITNESS_ENTRY_SIZE,
                actual: self.witness_chain_segment.len(),
                context: "witness_chain_segment must be a multiple of witness entry size"
                    .to_string(),
            });
        }
        validate_bounded_text(
            "commit_message",
            &self.commit_message,
            MAX_COMMIT_MESSAGE_BYTES,
        )?;
        Ok(())
    }
}

impl TaskId {
    pub fn validate(&self, field: &str) -> Result<(), MejepaInferError> {
        validate_non_empty_id(field, &self.0)
    }
}

impl TestId {
    pub fn validate(&self, field: &str) -> Result<(), MejepaInferError> {
        validate_non_empty_id(field, &self.0)
    }
}

impl SkillId {
    pub fn validate(&self, field: &str) -> Result<(), MejepaInferError> {
        validate_non_empty_id(field, &self.0)
    }
}

impl ChunkId {
    pub fn validate(&self, field: &str) -> Result<(), MejepaInferError> {
        validate_non_empty_id(field, &self.0)
    }
}

impl EmbedderId {
    pub fn validate(&self, field: &str) -> Result<(), MejepaInferError> {
        validate_non_empty_id(field, &self.0)
    }
}

impl AstDiff {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        if self.hunks.is_empty() {
            return Err(MejepaInferError::InvalidInput {
                field: "ast_diff.hunks".to_string(),
                detail: "ast diff must contain at least one hunk".to_string(),
            });
        }
        if self.hunks.len() > MAX_PATCH_HUNKS {
            return Err(MejepaInferError::DimMismatch {
                expected: MAX_PATCH_HUNKS,
                actual: self.hunks.len(),
                context: "ast_diff.hunks exceeds maximum".to_string(),
            });
        }
        for (idx, hunk) in self.hunks.iter().enumerate() {
            hunk.validate(&format!("ast_diff.hunks[{idx}]"))?;
        }
        Ok(())
    }
}

impl DiffHunk {
    pub fn validate(&self, field: &str) -> Result<(), MejepaInferError> {
        validate_relative_path(&format!("{field}.path"), &self.path)?;
        validate_bounded_text(
            &format!("{field}.before"),
            &self.before,
            MAX_HUNK_TEXT_BYTES,
        )?;
        validate_bounded_text(&format!("{field}.after"), &self.after, MAX_HUNK_TEXT_BYTES)
    }
}

impl TaskContext {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        self.task_id.validate("task_id")?;
        if self.session_id == [0u8; 16] {
            return Err(MejepaInferError::InvalidInput {
                field: "session_id".to_string(),
                detail: "session_id must be non-zero".to_string(),
            });
        }
        if self.problem_statement.trim().is_empty() {
            return Err(MejepaInferError::InvalidInput {
                field: "problem_statement".to_string(),
                detail: "problem_statement must be non-empty".to_string(),
            });
        }
        validate_bounded_text(
            "problem_statement",
            &self.problem_statement,
            MAX_PROBLEM_STATEMENT_BYTES,
        )?;
        if self.tests.is_empty() {
            return Err(MejepaInferError::InvalidInput {
                field: "tests".to_string(),
                detail: "at least one test id is required".to_string(),
            });
        }
        if self.tests.len() > MAX_TESTS {
            return Err(MejepaInferError::DimMismatch {
                expected: MAX_TESTS,
                actual: self.tests.len(),
                context: "tests exceeds maximum".to_string(),
            });
        }
        for (idx, test) in self.tests.iter().enumerate() {
            test.validate(&format!("tests[{idx}]"))?;
        }
        self.environment.validate()?;
        if let Some(claim_graph) = &self.claim_graph {
            claim_graph.validate()?;
        }
        if self.skill_citations.len() > MAX_SKILL_CITATIONS {
            return Err(MejepaInferError::DimMismatch {
                expected: MAX_SKILL_CITATIONS,
                actual: self.skill_citations.len(),
                context: "skill_citations exceeds maximum".to_string(),
            });
        }
        for (idx, citation) in self.skill_citations.iter().enumerate() {
            citation.validate(&format!("skill_citations[{idx}]"))?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskContext {
    pub task_id: TaskId,
    pub session_id: [u8; 16],
    pub language: Language,
    pub problem_statement: String,
    pub tests: Vec<TestId>,
    pub environment: TaskEnvironment,
    pub claim_graph: Option<ClaimGraph>,
    pub skill_citations: Vec<SkillCitation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskEnvironment {
    pub repo_root: PathBuf,
    pub python_version: Option<String>,
    pub os: String,
}

impl TaskEnvironment {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        if self.repo_root.as_os_str().is_empty() {
            return Err(MejepaInferError::InvalidInput {
                field: "environment.repo_root".to_string(),
                detail: "repo_root must be non-empty".to_string(),
            });
        }
        reject_path_control_chars("environment.repo_root", &self.repo_root)?;
        validate_non_empty_id("environment.os", &self.os)?;
        if let Some(version) = &self.python_version {
            validate_non_empty_id("environment.python_version", version)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimGraph {
    pub chunks: Vec<ChunkId>,
    pub entities: Vec<EntityType>,
}

impl ClaimGraph {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        if self.chunks.len() > MAX_CHUNKS {
            return Err(MejepaInferError::DimMismatch {
                expected: MAX_CHUNKS,
                actual: self.chunks.len(),
                context: "claim_graph.chunks exceeds maximum".to_string(),
            });
        }
        if self.entities.len() > MAX_ENTITIES {
            return Err(MejepaInferError::DimMismatch {
                expected: MAX_ENTITIES,
                actual: self.entities.len(),
                context: "claim_graph.entities exceeds maximum".to_string(),
            });
        }
        for (idx, chunk) in self.chunks.iter().enumerate() {
            chunk.validate(&format!("claim_graph.chunks[{idx}]"))?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SkillCitation {
    pub skill_id: SkillId,
    pub confidence_ppm: u32,
}

impl SkillCitation {
    pub fn validate(&self, field: &str) -> Result<(), MejepaInferError> {
        self.skill_id.validate(&format!("{field}.skill_id"))?;
        if self.confidence_ppm > 1_000_000 {
            return Err(MejepaInferError::InvalidInput {
                field: format!("{field}.confidence_ppm"),
                detail: format!(
                    "confidence_ppm must be <= 1000000, got {}",
                    self.confidence_ppm
                ),
            });
        }
        Ok(())
    }
}

// =============================================================================
// REQ-FLYWHEEL-10 — value types for `CF_MEJEPA_DDA_SIGNALS`,
// `CF_MEJEPA_FAILURE_EXEMPLARS`, and `CF_MEJEPA_AGENT_FEEDBACK`.
//
// All three structs persist as serde-JSON values keyed by bincode tuples.
// Each `validate()` is the fail-closed gate enforced before write — never
// store a value that round-trips into invalid state.
// =============================================================================

/// 128-bit prediction identifier (matches `RealityPrediction::prediction_id`).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
#[serde(transparent)]
pub struct PredictionId(pub [u8; 16]);

/// 128-bit feedback identifier; minted client-side so retries dedupe.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
#[serde(transparent)]
pub struct FeedbackId(pub [u8; 16]);

/// 256-bit panel content hash (matches `RealityPrediction::source_panel_sha`).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
#[serde(transparent)]
pub struct PanelId(pub [u8; 32]);

/// 256-bit witness chain hash binding the event to a tamper-evident segment.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
#[serde(transparent)]
pub struct WitnessHash(pub [u8; 32]);

/// Stable agent identifier (e.g. `"claude-opus-4.7"`, `"cursor-agent-v2"`).
/// String-shaped so non-Anthropic agents integrate without schema changes.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default)]
#[serde(transparent)]
pub struct AgentId(pub String);

impl AgentId {
    pub fn try_new(value: impl Into<String>) -> Result<Self, MejepaInferError> {
        let value = value.into();
        validate_bounded_id("agent_id", &value, MAX_AGENT_ID_BYTES)?;
        Ok(Self(value))
    }
}

/// REQ-FLYWHEEL-12 sampling-weight bump severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SurpriseSeverity {
    Low,
    Medium,
    High,
    Catastrophic,
}

impl SurpriseSeverity {
    /// `severity_score ∈ [0, 1]` per REQ-FLYWHEEL-12: sampling weight becomes
    /// `1 + 2 * severity_score`, so the bump stays in `[1, 3]`.
    pub fn severity_score(self) -> f32 {
        match self {
            Self::Low => 0.2,
            Self::Medium => 0.5,
            Self::High => 0.8,
            Self::Catastrophic => 1.0,
        }
    }
}

/// Agent's interpretation of how their lived reality compared to the
/// prediction. `Surprise` is the only variant that drives sampling bumps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeedbackKind {
    Confirmed,
    Surprise,
    Omission,
    Calibration,
}

/// What actually happened — measured, not predicted. Captured at the time the
/// agent files feedback so the train-side observer doesn't need to re-derive.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActualOutcome {
    pub oracle_outcome: OracleOutcome,
    pub failed_tests: Vec<TestId>,
    pub runtime_ms: Option<u64>,
    pub notes: String,
}

impl ActualOutcome {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        if self.failed_tests.len() > MAX_TESTS {
            return Err(MejepaInferError::InvalidInput {
                field: "actual_outcome.failed_tests".to_string(),
                detail: format!(
                    "failed_tests has {} entries; max supported is {}",
                    self.failed_tests.len(),
                    MAX_TESTS
                ),
            });
        }
        for (idx, test) in self.failed_tests.iter().enumerate() {
            validate_non_empty_id(&format!("actual_outcome.failed_tests[{idx}]"), &test.0)?;
        }
        if let Some(ms) = self.runtime_ms {
            if ms > 24 * 60 * 60 * 1000 {
                return Err(MejepaInferError::InvalidInput {
                    field: "actual_outcome.runtime_ms".to_string(),
                    detail: format!("runtime_ms must be <= 86_400_000; got {ms}"),
                });
            }
        }
        validate_bounded_text(
            "actual_outcome.notes",
            &self.notes,
            MAX_COMMIT_MESSAGE_BYTES,
        )?;
        Ok(())
    }
}

/// Coarse-grained label of how an attempt failed. Source of the label is
/// `TASK-PREDICT-LABEL-001` (pytest traceback parser); the corpus also writes
/// these labels when generating failure-mode-tagged mutations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureModeClass {
    AssertionMismatch,
    WrongAlgorithm,
    OffByOne,
    NullDeref,
    ResourceLeak,
    RaceCondition,
    DeadlockPotential,
    InfiniteLoop,
    StackOverflow,
    Exception,
    TypeError,
    SignatureMismatch,
    ImportMissing,
    SyntaxError,
    SemanticError,
    AssertionViolation,
    ContractViolation,
    ApiMisuse,
    ConfigDrift,
    EncodingError,
    DateTimeError,
    NumericalInstability,
    SerializationError,
    SchemaMigrationGap,
    DependencyVersionConflict,
    PlatformAssumption,
    CompilerVersionGap,
    PermissionsError,
    WrongTestUpdated,
    NameError,
    Timeout,
    Crash,
    CompileError,
    Flaky,
    NoTestsCollected,
    Other,
}

impl FailureModeClass {
    pub fn all() -> [Self; 36] {
        [
            Self::AssertionMismatch,
            Self::WrongAlgorithm,
            Self::OffByOne,
            Self::NullDeref,
            Self::ResourceLeak,
            Self::RaceCondition,
            Self::DeadlockPotential,
            Self::InfiniteLoop,
            Self::StackOverflow,
            Self::Exception,
            Self::TypeError,
            Self::SignatureMismatch,
            Self::ImportMissing,
            Self::SyntaxError,
            Self::SemanticError,
            Self::AssertionViolation,
            Self::ContractViolation,
            Self::ApiMisuse,
            Self::ConfigDrift,
            Self::EncodingError,
            Self::DateTimeError,
            Self::NumericalInstability,
            Self::SerializationError,
            Self::SchemaMigrationGap,
            Self::DependencyVersionConflict,
            Self::PlatformAssumption,
            Self::CompilerVersionGap,
            Self::PermissionsError,
            Self::WrongTestUpdated,
            Self::NameError,
            Self::Timeout,
            Self::Crash,
            Self::CompileError,
            Self::Flaky,
            Self::NoTestsCollected,
            Self::Other,
        ]
    }

    pub fn slug(self) -> &'static str {
        match self {
            Self::AssertionMismatch => "assertion_mismatch",
            Self::WrongAlgorithm => "wrong_algorithm",
            Self::OffByOne => "off_by_one",
            Self::NullDeref => "null_deref",
            Self::ResourceLeak => "resource_leak",
            Self::RaceCondition => "race_condition",
            Self::DeadlockPotential => "deadlock_potential",
            Self::InfiniteLoop => "infinite_loop",
            Self::StackOverflow => "stack_overflow",
            Self::Exception => "exception",
            Self::TypeError => "type_error",
            Self::SignatureMismatch => "signature_mismatch",
            Self::ImportMissing => "import_missing",
            Self::SyntaxError => "syntax_error",
            Self::SemanticError => "semantic_error",
            Self::AssertionViolation => "assertion_violation",
            Self::ContractViolation => "contract_violation",
            Self::ApiMisuse => "api_misuse",
            Self::ConfigDrift => "config_drift",
            Self::EncodingError => "encoding_error",
            Self::DateTimeError => "date_time_error",
            Self::NumericalInstability => "numerical_instability",
            Self::SerializationError => "serialization_error",
            Self::SchemaMigrationGap => "schema_migration_gap",
            Self::DependencyVersionConflict => "dependency_version_conflict",
            Self::PlatformAssumption => "platform_assumption",
            Self::CompilerVersionGap => "compiler_version_gap",
            Self::PermissionsError => "permissions_error",
            Self::WrongTestUpdated => "wrong_test_updated",
            Self::NameError => "name_error",
            Self::Timeout => "timeout",
            Self::Crash => "crash",
            Self::CompileError => "compile_error",
            Self::Flaky => "flaky",
            Self::NoTestsCollected => "no_tests_collected",
            Self::Other => "other",
        }
    }
}

/// Deterministic signature of which embedders flagged a chunk-level violation.
/// Canonicalized to a sorted distinct `Vec<EmbedderId>` so semantically-equal
/// signature sets hash to the same CF key.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct EmbedderViolationSignature {
    pub embedder_ids: Vec<EmbedderId>,
}

impl EmbedderViolationSignature {
    pub fn canonicalize(mut self) -> Result<Self, MejepaInferError> {
        if self.embedder_ids.len() > MAX_EMBEDDERS_PER_PANEL {
            return Err(MejepaInferError::InvalidInput {
                field: "embedder_violation_signature.embedder_ids".to_string(),
                detail: format!(
                    "{} embedder ids; max is {}",
                    self.embedder_ids.len(),
                    MAX_EMBEDDERS_PER_PANEL
                ),
            });
        }
        for (idx, eid) in self.embedder_ids.iter().enumerate() {
            validate_non_empty_id(
                &format!("embedder_violation_signature.embedder_ids[{idx}]"),
                &eid.0,
            )?;
        }
        self.embedder_ids
            .sort_by(|a, b| a.0.as_str().cmp(b.0.as_str()));
        self.embedder_ids.dedup();
        Ok(self)
    }
}

/// CF_MEJEPA_DDA_SIGNALS value: per-embedder + pairwise signal block per chunk.
///
/// Invariants enforced by `validate`:
/// - `per_embedder_cosine.len() == N` where `N` is the active-embedder count
/// - `pairwise_cosine_upper.len() == N * (N - 1) / 2` (strict upper triangle)
/// - `blind_spot_z_scores.len() == pairwise_cosine_upper.len()`
/// - `pairwise_mi_upper` is either empty (no weekly MI computed yet) or matches
///   the upper-triangle length above (cached-weekly per TECH-FLYWHEEL.md §6)
/// - every float is finite; cosines clamp to `[-1.0, 1.0]`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct DdaSignals {
    pub per_embedder_cosine: Vec<f32>,
    pub pairwise_cosine_upper: Vec<f32>,
    pub pairwise_mi_upper: Vec<f32>,
    pub blind_spot_z_scores: Vec<f32>,
}

impl DdaSignals {
    pub fn try_new(value: Self) -> Result<Self, MejepaInferError> {
        value.validate()?;
        Ok(value)
    }

    pub fn embedder_count(&self) -> usize {
        self.per_embedder_cosine.len()
    }

    pub fn expected_upper_triangle_len(&self) -> usize {
        let n = self.embedder_count();
        if n < 2 {
            0
        } else {
            n * (n - 1) / 2
        }
    }

    pub fn validate(&self) -> Result<(), MejepaInferError> {
        let n = self.per_embedder_cosine.len();
        if n > MAX_EMBEDDERS_PER_PANEL {
            return Err(MejepaInferError::InvalidInput {
                field: "dda_signals.per_embedder_cosine".to_string(),
                detail: format!(
                    "per_embedder_cosine has {n} entries; max is {MAX_EMBEDDERS_PER_PANEL}",
                ),
            });
        }
        for (idx, value) in self.per_embedder_cosine.iter().enumerate() {
            validate_cosine(&format!("dda_signals.per_embedder_cosine[{idx}]"), *value)?;
        }
        let expected = self.expected_upper_triangle_len();
        if self.pairwise_cosine_upper.len() != expected {
            return Err(MejepaInferError::DimMismatch {
                expected,
                actual: self.pairwise_cosine_upper.len(),
                context: "dda_signals.pairwise_cosine_upper must equal N*(N-1)/2".to_string(),
            });
        }
        if expected > MAX_PAIRWISE_SIGNALS {
            return Err(MejepaInferError::InvalidInput {
                field: "dda_signals.pairwise_cosine_upper".to_string(),
                detail: format!(
                    "{expected} pairwise entries exceed the MAX_PAIRWISE_SIGNALS cap of {MAX_PAIRWISE_SIGNALS}",
                ),
            });
        }
        for (idx, value) in self.pairwise_cosine_upper.iter().enumerate() {
            validate_cosine(&format!("dda_signals.pairwise_cosine_upper[{idx}]"), *value)?;
        }
        if !self.pairwise_mi_upper.is_empty() && self.pairwise_mi_upper.len() != expected {
            return Err(MejepaInferError::DimMismatch {
                expected,
                actual: self.pairwise_mi_upper.len(),
                context: "dda_signals.pairwise_mi_upper must equal N*(N-1)/2 or be empty"
                    .to_string(),
            });
        }
        for (idx, value) in self.pairwise_mi_upper.iter().enumerate() {
            if !value.is_finite() || *value < 0.0 {
                return Err(MejepaInferError::NanDetected {
                    nan_source: format!("dda_signals.pairwise_mi_upper[{idx}]"),
                    detail: format!("mi must be finite and non-negative; got {value}"),
                });
            }
        }
        if self.blind_spot_z_scores.len() != expected {
            return Err(MejepaInferError::DimMismatch {
                expected,
                actual: self.blind_spot_z_scores.len(),
                context: "dda_signals.blind_spot_z_scores must equal N*(N-1)/2".to_string(),
            });
        }
        for (idx, value) in self.blind_spot_z_scores.iter().enumerate() {
            if !value.is_finite() {
                return Err(MejepaInferError::NanDetected {
                    nan_source: format!("dda_signals.blind_spot_z_scores[{idx}]"),
                    detail: format!("z-score must be finite; got {value}"),
                });
            }
        }
        Ok(())
    }
}

/// CF_MEJEPA_FAILURE_EXEMPLARS row. Each row points at a concrete chunk that
/// exhibited the keyed `(FailureModeClass, EmbedderViolationSignature)`; the
/// CF value is `Vec<ExemplarRef>`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExemplarRef {
    pub task_id: TaskId,
    pub mutation_kind: crate::eval::MutationCategory,
    pub chunk_id: ChunkId,
    pub oracle_verdict: OracleVerdict,
    pub witness_hash: WitnessHash,
}

impl ExemplarRef {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        validate_non_empty_id("exemplar_ref.task_id", &self.task_id.0)?;
        validate_non_empty_id("exemplar_ref.chunk_id", &self.chunk_id.0)?;
        if self.oracle_verdict.per_test.is_empty() && self.oracle_verdict.exception.is_none() {
            return Err(MejepaInferError::InvalidInput {
                field: "exemplar_ref.oracle_verdict".to_string(),
                detail: "oracle_verdict must record at least one test outcome or an exception"
                    .to_string(),
            });
        }
        if self.oracle_verdict.per_test.len() > MAX_TESTS {
            return Err(MejepaInferError::InvalidInput {
                field: "exemplar_ref.oracle_verdict.per_test".to_string(),
                detail: format!(
                    "per_test has {} entries; max is {}",
                    self.oracle_verdict.per_test.len(),
                    MAX_TESTS
                ),
            });
        }
        Ok(())
    }
}

/// Validate the full `Vec<ExemplarRef>` payload before persistence.
pub fn validate_exemplar_bucket(exemplars: &[ExemplarRef]) -> Result<(), MejepaInferError> {
    if exemplars.len() > MAX_EXEMPLARS_PER_BUCKET {
        return Err(MejepaInferError::InvalidInput {
            field: "exemplar_bucket".to_string(),
            detail: format!(
                "{} exemplars in one bucket; max is {}",
                exemplars.len(),
                MAX_EXEMPLARS_PER_BUCKET
            ),
        });
    }
    for (idx, exemplar) in exemplars.iter().enumerate() {
        exemplar.validate().map_err(|err| match err {
            MejepaInferError::InvalidInput { field, detail } => MejepaInferError::InvalidInput {
                field: format!("exemplar_bucket[{idx}].{field}"),
                detail,
            },
            other => other,
        })?;
    }
    Ok(())
}

/// CF_MEJEPA_AGENT_FEEDBACK value: append-only ledger entry recording an
/// agent's interpretation of a prior `RealityPrediction`. Keyed by
/// `(prediction_id, agent_id, ts_millis)` so per-prediction lookups are O(1)
/// and time-ordered iteration is the natural CF iterator order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SurpriseEvent {
    pub feedback_id: FeedbackId,
    pub prediction_id: PredictionId,
    pub agent_id: AgentId,
    /// Milliseconds since Unix epoch — stable cross-process serialization. The
    /// key encoder uses this same value so primary key and payload stay in
    /// sync across restore.
    pub ts_millis: i64,
    pub feedback_kind: FeedbackKind,
    pub agent_explanation: String,
    pub actual_outcome: Option<ActualOutcome>,
    pub severity: SurpriseSeverity,
    pub extra_structured_data: serde_json::Value,
    pub witness_hash: WitnessHash,
}

impl SurpriseEvent {
    pub fn try_new(value: Self) -> Result<Self, MejepaInferError> {
        value.validate()?;
        Ok(value)
    }

    pub fn validate(&self) -> Result<(), MejepaInferError> {
        if self.agent_id.0.trim().is_empty() {
            return Err(MejepaInferError::InvalidInput {
                field: "surprise_event.agent_id".to_string(),
                detail: "agent_id must be non-empty".to_string(),
            });
        }
        validate_bounded_id(
            "surprise_event.agent_id",
            &self.agent_id.0,
            MAX_AGENT_ID_BYTES,
        )?;
        if self.ts_millis < 0 {
            return Err(MejepaInferError::InvalidInput {
                field: "surprise_event.ts_millis".to_string(),
                detail: format!("ts_millis must be non-negative; got {}", self.ts_millis),
            });
        }
        validate_bounded_text(
            "surprise_event.agent_explanation",
            &self.agent_explanation,
            MAX_AGENT_EXPLANATION_BYTES,
        )?;
        if let Some(outcome) = &self.actual_outcome {
            outcome.validate()?;
        }
        if matches!(
            self.feedback_kind,
            FeedbackKind::Surprise | FeedbackKind::Omission
        ) && self.actual_outcome.is_none()
        {
            return Err(MejepaInferError::InvalidInput {
                field: "surprise_event.actual_outcome".to_string(),
                detail: format!(
                    "actual_outcome is required when feedback_kind={:?}",
                    self.feedback_kind
                ),
            });
        }
        let payload_bytes = serde_json::to_vec(&self.extra_structured_data).map_err(|err| {
            MejepaInferError::InvalidInput {
                field: "surprise_event.extra_structured_data".to_string(),
                detail: format!("extra_structured_data must serialize to JSON: {err}"),
            }
        })?;
        if payload_bytes.len() > MAX_AGENT_FEEDBACK_PAYLOAD_BYTES {
            return Err(MejepaInferError::InvalidInput {
                field: "surprise_event.extra_structured_data".to_string(),
                detail: format!(
                    "extra_structured_data serialized to {} bytes; max is {}",
                    payload_bytes.len(),
                    MAX_AGENT_FEEDBACK_PAYLOAD_BYTES
                ),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityType {
    Function,
    Class,
    Module,
    Test,
    File,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SideEffect {
    FileWrite,
    Network,
    ProcessSpawn,
    DatabaseWrite,
}

fn validate_items<T>(
    field: &str,
    items: &[T],
    validate: impl Fn(&T) -> Result<(), MejepaInferError>,
) -> Result<(), MejepaInferError> {
    for (idx, item) in items.iter().enumerate() {
        validate(item).map_err(|err| match err {
            MejepaInferError::InvalidInput {
                field: subfield,
                detail,
            } => MejepaInferError::InvalidInput {
                field: format!("{field}[{idx}].{subfield}"),
                detail,
            },
            MejepaInferError::DimMismatch {
                expected,
                actual,
                context,
            } => MejepaInferError::DimMismatch {
                expected,
                actual,
                context: format!("{field}[{idx}].{context}"),
            },
            other => other,
        })?;
    }
    Ok(())
}

fn validate_line_range(field: &str, range: (u32, u32)) -> Result<(), MejepaInferError> {
    if range.0 == 0 || range.1 == 0 || range.0 > range.1 {
        return Err(MejepaInferError::InvalidInput {
            field: field.to_string(),
            detail: format!(
                "line range must be one-indexed and start <= end; got {}..{}",
                range.0, range.1
            ),
        });
    }
    Ok(())
}

fn validate_optional_id(field: &str, value: &str) -> Result<(), MejepaInferError> {
    if value.is_empty() {
        return Ok(());
    }
    validate_non_empty_id(field, value)
}

fn validate_paths(field: &str, paths: &[PathBuf]) -> Result<(), MejepaInferError> {
    if paths.len() > MAX_PATCH_HUNKS {
        return Err(MejepaInferError::DimMismatch {
            expected: MAX_PATCH_HUNKS,
            actual: paths.len(),
            context: format!("{field} exceeds maximum"),
        });
    }
    for (idx, path) in paths.iter().enumerate() {
        validate_relative_path(&format!("{field}[{idx}]"), path)?;
    }
    Ok(())
}

fn validate_finite_f32(field: &str, value: f32) -> Result<(), MejepaInferError> {
    if !value.is_finite() {
        return Err(MejepaInferError::NanDetected {
            nan_source: field.to_string(),
            detail: format!("{field} must be finite; got {value}"),
        });
    }
    Ok(())
}

fn validate_finite_f64(field: &str, value: f64) -> Result<(), MejepaInferError> {
    if !value.is_finite() {
        return Err(MejepaInferError::NanDetected {
            nan_source: field.to_string(),
            detail: format!("{field} must be finite; got {value}"),
        });
    }
    Ok(())
}

fn validate_optional_f64(field: &str, value: Option<f64>) -> Result<(), MejepaInferError> {
    if let Some(value) = value {
        validate_finite_f64(field, value)?;
    }
    Ok(())
}

pub(crate) fn validate_probability(field: &str, value: f32) -> Result<(), MejepaInferError> {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return Err(MejepaInferError::NanDetected {
            nan_source: field.to_string(),
            detail: format!("{field} must be finite and in [0, 1]; got {value}"),
        });
    }
    Ok(())
}

fn validate_optional_probability(field: &str, value: Option<f32>) -> Result<(), MejepaInferError> {
    if let Some(value) = value {
        validate_probability(field, value)?;
    }
    Ok(())
}

fn validate_non_empty_id(field: &str, value: &str) -> Result<(), MejepaInferError> {
    validate_bounded_id(field, value, MAX_ID_BYTES)
}

fn validate_bounded_id(field: &str, value: &str, max_bytes: usize) -> Result<(), MejepaInferError> {
    if value.trim().is_empty() {
        return Err(MejepaInferError::InvalidInput {
            field: field.to_string(),
            detail: "must be non-empty".to_string(),
        });
    }
    if value.len() > max_bytes {
        return Err(MejepaInferError::InvalidInput {
            field: field.to_string(),
            detail: format!("exceeds {max_bytes} bytes"),
        });
    }
    if value.bytes().any(|b| b < 0x20 || b == 0x7f) {
        return Err(MejepaInferError::InvalidInput {
            field: field.to_string(),
            detail: "contains a control character".to_string(),
        });
    }
    Ok(())
}

fn validate_id_list(
    field: &str,
    values: &[String],
    max_items: usize,
) -> Result<(), MejepaInferError> {
    if values.len() > max_items {
        return Err(MejepaInferError::DimMismatch {
            expected: max_items,
            actual: values.len(),
            context: format!("{field} exceeds maximum"),
        });
    }
    let mut seen = BTreeSet::new();
    for (idx, value) in values.iter().enumerate() {
        validate_bounded_id(&format!("{field}[{idx}]"), value, MAX_ID_BYTES)?;
        if !seen.insert(value) {
            return Err(MejepaInferError::InvalidInput {
                field: format!("{field}[{idx}]"),
                detail: "duplicate id".to_string(),
            });
        }
    }
    Ok(())
}

fn validate_cosine(field: &str, value: f32) -> Result<(), MejepaInferError> {
    if !value.is_finite() {
        return Err(MejepaInferError::NanDetected {
            nan_source: field.to_string(),
            detail: format!("{field} is {value}; must be finite"),
        });
    }
    if !(-1.0..=1.0).contains(&value) {
        return Err(MejepaInferError::InvalidInput {
            field: field.to_string(),
            detail: format!("cosine value out of [-1, 1]; got {value}"),
        });
    }
    Ok(())
}

fn validate_bounded_text(
    field: &str,
    value: &str,
    max_bytes: usize,
) -> Result<(), MejepaInferError> {
    if value.len() > max_bytes {
        return Err(MejepaInferError::InvalidInput {
            field: field.to_string(),
            detail: format!("exceeds {max_bytes} bytes"),
        });
    }
    reject_control_chars(field, value)
}

fn reject_control_chars(field: &str, value: &str) -> Result<(), MejepaInferError> {
    if value
        .bytes()
        .any(|b| (b < 0x20 && !matches!(b, b'\n' | b'\r' | b'\t')) || b == 0x7f)
    {
        return Err(MejepaInferError::InvalidInput {
            field: field.to_string(),
            detail: "contains a control character".to_string(),
        });
    }
    Ok(())
}

fn validate_relative_path(field: &str, path: &Path) -> Result<(), MejepaInferError> {
    if path.as_os_str().is_empty() {
        return Err(MejepaInferError::InvalidInput {
            field: field.to_string(),
            detail: "path must be non-empty".to_string(),
        });
    }
    if path.is_absolute() {
        return Err(MejepaInferError::InvalidInput {
            field: field.to_string(),
            detail: "path must be relative to repo_root".to_string(),
        });
    }
    for component in path.components() {
        if matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        ) {
            return Err(MejepaInferError::InvalidInput {
                field: field.to_string(),
                detail: "path must not escape repo_root".to_string(),
            });
        }
    }
    reject_path_control_chars(field, path)
}

fn reject_path_control_chars(field: &str, path: &Path) -> Result<(), MejepaInferError> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        if path
            .as_os_str()
            .as_bytes()
            .iter()
            .any(|b| *b < 0x20 || *b == 0x7f)
        {
            return Err(MejepaInferError::InvalidInput {
                field: field.to_string(),
                detail: "path contains a control character".to_string(),
            });
        }
    }
    #[cfg(not(unix))]
    {
        if path
            .to_string_lossy()
            .bytes()
            .any(|b| b < 0x20 || b == 0x7f)
        {
            return Err(MejepaInferError::InvalidInput {
                field: field.to_string(),
                detail: "path contains a control character".to_string(),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prediction_for_decode() -> RealityPrediction {
        RealityPredictionBuilder::from_parts(
            TaskId("decode-compat-task".to_string()),
            [1; 16],
            Language::Python,
            ConformalSet::try_new(vec![OracleOutcome::Pass], 0.1, 0.2).unwrap(),
        )
        .prediction_id([2; 16])
        .witness_hash(WitnessHash([3; 32]))
        .verdict(Verdict::Pass)
        .confidence_interval(ConformalInterval {
            lower: 0.6,
            upper: 0.8,
            ..ConformalInterval::default()
        })
        .predicted_oracle_pass(0.9)
        .predicted_test_pass(vec![0.9])
        .ood_score(0.1)
        .calibrated_confidence(0.7)
        .granger_attestations(BTreeMap::from([("decode:unit".to_string(), 0.9)]))
        .provenance(PredictionProvenance {
            predictor_version: "decode-test".to_string(),
            constellation_version: "decode-test-constellation".to_string(),
            calibration_version: "calibration-v1".to_string(),
            active_pointer: hex::encode([3; 16]),
            train_health_source: String::new(),
        })
        .source_panel_sha([4; 32])
        .calibration_version("calibration-v1")
        .created_at_unix_ms(1_772_000_000_000)
        .label_context(PredictionLabelContext {
            accepted_label_ids: vec!["ast_surface:function".to_string()],
            code_state_key: Some("python:before:unit".to_string()),
            failure_evidence_set_ids: vec!["evidence:unit".to_string()],
            active_skill_ids: vec!["skill:unit_sequence".to_string()],
            accepted_registry_sha256: Some("sha256:abc123".to_string()),
            usefulness_metrics_sha256: Some("sha256:def456".to_string()),
            learning_bridge_manifest_sha256: Some("sha256:fedcba".to_string()),
            label_signature_hash: Some("labels:abc123def456".to_string()),
            skill_signature_hash: Some("skills:654fed321cba".to_string()),
            ..PredictionLabelContext::default()
        })
        .build()
        .unwrap()
    }

    #[test]
    fn decode_reality_prediction_reads_legacy_without_constellation_field() {
        let prediction = prediction_for_decode();
        let legacy = RealityPredictionLegacyNoConstellation::from_current(&prediction);
        let legacy_bytes = bincode::serialize(&legacy).unwrap();

        let decoded = decode_reality_prediction(&legacy_bytes).unwrap();

        assert_eq!(decoded.prediction_id, prediction.prediction_id);
        assert_eq!(decoded.task_id, prediction.task_id);
        assert_eq!(decoded.constellation_intelligence, None);
        assert!(!decoded.slot_attributions.is_empty());
    }

    #[test]
    fn decode_reality_prediction_reads_legacy_without_slot_attributions_field() {
        let prediction = prediction_for_decode();
        let legacy = RealityPredictionLegacyNoSlotAttributions::from_current(&prediction);
        let legacy_bytes = bincode::serialize(&legacy).unwrap();

        let decoded = decode_reality_prediction(&legacy_bytes).unwrap();

        assert_eq!(decoded.prediction_id, prediction.prediction_id);
        assert_eq!(
            decoded.constellation_intelligence,
            prediction.constellation_intelligence
        );
        assert!(!decoded.slot_attributions.is_empty());
    }

    #[test]
    fn decode_reality_prediction_reads_legacy_without_label_context_field() {
        let prediction = prediction_for_decode();
        let legacy = RealityPredictionLegacyNoLabelContext::from_current(&prediction);
        let legacy_bytes = bincode::serialize(&legacy).unwrap();

        let decoded = decode_reality_prediction(&legacy_bytes).unwrap();

        assert_eq!(decoded.prediction_id, prediction.prediction_id);
        assert_eq!(decoded.label_context, PredictionLabelContext::default());
        assert!(!decoded.slot_attributions.is_empty());
    }

    #[test]
    fn decode_reality_prediction_reads_legacy_v1_label_context() {
        let prediction = prediction_for_decode();
        let legacy_prediction = RealityPredictionLegacyNoLabelContext::from_current(&prediction);
        let legacy_label = PredictionLabelContextLegacyV1::from_current(&prediction.label_context);
        let mut bytes = bincode::serialize(&legacy_prediction).unwrap();
        bytes.extend(bincode::serialize(&legacy_label).unwrap());

        let decoded = decode_reality_prediction(&bytes).unwrap();

        assert_eq!(decoded.prediction_id, prediction.prediction_id);
        assert_eq!(
            decoded.label_context.accepted_label_ids,
            prediction.label_context.accepted_label_ids
        );
        assert_eq!(
            decoded.label_context.active_skill_ids,
            prediction.label_context.active_skill_ids
        );
        assert!(decoded.label_context.active_higher_ability_ids.is_empty());
        assert!(decoded.label_context.source_membership_keys.is_empty());
        assert!(decoded.label_context.ability_signature_hash.is_none());
        assert!(decoded.label_context.membership_signature_hash.is_none());
    }

    #[test]
    fn builder_clears_q4_display_only_fields_for_new_predictions() {
        let chunk = ChunkId("src/unit.py#0".to_string());
        let mut surfaces = PhaseBPredictionSurfaces::default();
        surfaces
            .predicted_perf_regressions
            .push(PredictedPerfRegression {
                axis: PerfAxis::CpuMs,
                chunk: chunk.clone(),
                predicted_delta_pct: 12.0,
                baseline_value: None,
                confidence: 0.8,
                explanation: "legacy Q4 perf observation".to_string(),
            });
        surfaces
            .predicted_security_concerns
            .push(PredictedSecurityConcern {
                class: SecurityConcernClass::HardcodedSecret,
                chunk,
                line_range: (1, 1),
                cvss_estimate: Some(7.5),
                explanation: "legacy Q4 security observation".to_string(),
            });
        surfaces.predicted_reasoning_class = ReasoningClass::Overclaiming;

        let prediction = RealityPredictionBuilder::from_parts(
            TaskId("q4-freeze-build".to_string()),
            [0x61; 16],
            Language::Python,
            ConformalSet::try_new(vec![OracleOutcome::Pass], 0.1, 0.2).unwrap(),
        )
        .prediction_id([0x62; 16])
        .witness_hash(WitnessHash([0x63; 32]))
        .verdict(Verdict::Pass)
        .confidence_interval(ConformalInterval {
            lower: 0.6,
            upper: 0.8,
            ..ConformalInterval::default()
        })
        .predicted_oracle_pass(0.8)
        .predicted_test_pass(vec![0.8])
        .ood_score(0.1)
        .calibrated_confidence(0.8)
        .phase_b_surfaces(surfaces)
        .provenance(PredictionProvenance {
            predictor_version: "q4-freeze-build".to_string(),
            constellation_version: "q4-freeze-build".to_string(),
            calibration_version: "q4-freeze-build".to_string(),
            active_pointer: "q4-freeze-build".to_string(),
            train_health_source: String::new(),
        })
        .source_panel_sha([0x64; 32])
        .calibration_version("q4-freeze-build")
        .build()
        .unwrap();

        assert!(prediction.q4_display_only_fields_empty());
        assert!(prediction
            .slot_attributions
            .iter()
            .all(|item| item.source != SlotAttributionSource::Q4Head));
    }

    #[test]
    fn decode_reality_prediction_rejects_trailing_bytes_instead_of_legacy_prefix() {
        let prediction = prediction_for_decode();
        let mut bytes = bincode::serialize(&prediction).unwrap();
        bytes.extend_from_slice(&[0xde, 0xad, 0xbe, 0xef]);

        let err = decode_reality_prediction(&bytes).unwrap_err();

        assert!(matches!(err, MejepaInferError::InvalidInput { .. }));
    }

    #[test]
    fn decode_hierarchical_prediction_reads_legacy_without_slot_attributions_field() {
        let chunk = ChunkId("src/unit.py#0".to_string());
        let record = HierarchicalPredictionRecord::try_new(HierarchicalPredictionRecord {
            schema_version: HIERARCHICAL_PREDICTION_SCHEMA_VERSION,
            prediction_id: [0x21; 16],
            task_id: TaskId("hierarchy-decode-task".to_string()),
            session_id: [0x22; 16],
            language: Language::Python,
            source_panel_sha: [0x23; 32],
            calibration_version: "hierarchy-decode".to_string(),
            created_at_unix_ms: 1_772_000_000_000,
            slot_attributions: vec![unit_slot_attribution(SlotAttributionPolarity::Supporting)],
            levels: hierarchy_decode_levels(chunk),
        })
        .unwrap();
        let legacy = HierarchicalPredictionRecordLegacyNoSlotAttributions::from_current(&record);
        let legacy_bytes = bincode::serialize(&legacy).unwrap();

        let decoded = decode_hierarchical_prediction_record(&legacy_bytes).unwrap();

        assert_eq!(decoded.prediction_id, record.prediction_id);
        assert!(decoded.slot_attributions.is_empty());
        assert_eq!(decoded.levels, record.levels);
    }

    fn hierarchy_decode_levels(chunk: ChunkId) -> Vec<HierarchicalPredictionLevel> {
        let file = "file:src/unit.py".to_string();
        let function = "file:src/unit.py/function:unit#0".to_string();
        let ast = "file:src/unit.py/function:unit#0/ast:function:abc".to_string();
        vec![
            hierarchy_decode_level(
                PredictionHierarchyLevel::File,
                file.clone(),
                None,
                chunk.clone(),
            ),
            hierarchy_decode_level(
                PredictionHierarchyLevel::Function,
                function.clone(),
                Some(file),
                chunk.clone(),
            ),
            hierarchy_decode_level(
                PredictionHierarchyLevel::AstNode,
                ast.clone(),
                Some(function),
                chunk.clone(),
            ),
            hierarchy_decode_level(
                PredictionHierarchyLevel::Chunk,
                "file:src/unit.py/function:unit#0/ast:function:abc/chunk:0".to_string(),
                Some(ast),
                chunk,
            ),
        ]
    }

    fn hierarchy_decode_level(
        level: PredictionHierarchyLevel,
        scope_id: String,
        parent_scope_id: Option<String>,
        chunk: ChunkId,
    ) -> HierarchicalPredictionLevel {
        HierarchicalPredictionLevel {
            level,
            scope_id,
            parent_scope_id,
            covered_chunks: vec![chunk],
            predicted_oracle_pass: 0.7,
            calibrated_confidence: 0.8,
            ood_score: 0.1,
            verdict: Verdict::Pass,
            latent_energy: 0.01,
        }
    }

    #[test]
    fn slot_attributions_reject_duplicate_evidence() {
        let attr = unit_slot_attribution(SlotAttributionPolarity::Supporting);
        let err = RealityPredictionBuilder::from_parts(
            TaskId("slot-duplicate-task".to_string()),
            [0x31; 16],
            Language::Python,
            ConformalSet::try_new(vec![OracleOutcome::Pass], 0.1, 0.2).unwrap(),
        )
        .prediction_id([0x32; 16])
        .witness_hash(WitnessHash([0x33; 32]))
        .verdict(Verdict::Pass)
        .confidence_interval(ConformalInterval {
            lower: 0.6,
            upper: 0.8,
            ..ConformalInterval::default()
        })
        .predicted_oracle_pass(0.75)
        .predicted_test_pass(vec![0.75])
        .ood_score(0.1)
        .calibrated_confidence(0.7)
        .provenance(PredictionProvenance {
            predictor_version: "slot-test".to_string(),
            constellation_version: "slot-test".to_string(),
            calibration_version: "slot-test".to_string(),
            active_pointer: "slot-test".to_string(),
            train_health_source: String::new(),
        })
        .source_panel_sha([0x34; 32])
        .calibration_version("slot-test")
        .slot_attributions(vec![attr.clone(), attr])
        .build()
        .unwrap_err();
        assert_eq!(err.code(), "MEJEPA_INFER_INVALID_INPUT");
    }

    #[test]
    fn rejection_verdict_requires_rejection_slot_evidence() {
        let err = RealityPredictionBuilder::from_parts(
            TaskId("slot-rejection-task".to_string()),
            [0x41; 16],
            Language::Python,
            ConformalSet::try_new(vec![OracleOutcome::Fail], 0.1, 0.2).unwrap(),
        )
        .prediction_id([0x42; 16])
        .witness_hash(WitnessHash([0x43; 32]))
        .verdict(Verdict::GuardRejected)
        .confidence_interval(ConformalInterval {
            lower: 0.1,
            upper: 0.3,
            ..ConformalInterval::default()
        })
        .predicted_oracle_pass(0.2)
        .predicted_test_pass(vec![0.2])
        .ood_score(0.9)
        .calibrated_confidence(0.4)
        .provenance(PredictionProvenance {
            predictor_version: "slot-test".to_string(),
            constellation_version: "slot-test".to_string(),
            calibration_version: "slot-test".to_string(),
            active_pointer: "slot-test".to_string(),
            train_health_source: String::new(),
        })
        .source_panel_sha([0x44; 32])
        .calibration_version("slot-test")
        .slot_attributions(vec![unit_slot_attribution(
            SlotAttributionPolarity::Supporting,
        )])
        .build()
        .unwrap_err();
        assert_eq!(err.code(), "MEJEPA_INFER_INVALID_INPUT");
    }

    fn unit_slot_attribution(polarity: SlotAttributionPolarity) -> SlotAttributionEvidence {
        SlotAttributionEvidence {
            schema_version: SLOT_ATTRIBUTION_SCHEMA_VERSION,
            slot_id: "unit_slot".to_string(),
            embedder: Some(EmbedderId("E_UNIT".to_string())),
            chunk: Some(ChunkId("src/unit.py#0".to_string())),
            polarity,
            source: SlotAttributionSource::VerdictHead,
            score: 0.8,
            threshold: None,
            margin: None,
            reason: "unit attribution".to_string(),
            relationship_slot_id: None,
            related_fingerprint_id: None,
            active_learning_candidate_id: None,
            q_head: Some("q2_verdict".to_string()),
            impact_kind: None,
            evidence: "unit evidence".to_string(),
        }
    }
}
