use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

use context_graph_mejepa_instruments::OracleVerdict;
use serde::{Deserialize, Serialize};

use crate::error::MejepaInferError;

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
const MAX_HUNK_TEXT_BYTES: usize = 1_048_576;
const MAX_WITNESS_SEGMENT_BYTES: usize = 73 * 8192;
const MAX_COMMIT_MESSAGE_BYTES: usize = 8192;
const MAX_PROBLEM_STATEMENT_BYTES: usize = 65_536;

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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RealityPrediction {
    pub prediction_id: [u8; 16],
    pub task_id: TaskId,
    pub session_id: [u8; 16],
    pub language: Language,
    pub predicted_oracle_pass: f32,
    pub predicted_test_pass: Vec<f32>,
    pub predicted_runtime_trace: [f32; 32],
    pub ood_score: f32,
    pub outcome_set: ConformalSet,
    pub calibrated_confidence: f32,
    pub degraded_status: bool,
    pub granger_attestations: BTreeMap<String, f32>,
    pub source_panel_sha: [u8; 32],
    pub calibration_version: String,
    pub created_at_unix_ms: i64,
}

impl RealityPrediction {
    pub fn try_new(mut value: Self) -> Result<Self, MejepaInferError> {
        value.validate()?;
        value.predicted_test_pass.shrink_to_fit();
        Ok(value)
    }

    pub fn validate(&self) -> Result<(), MejepaInferError> {
        validate_non_empty_id("task_id", &self.task_id.0)?;
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
        self.outcome_set.validate()?;
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

#[derive(Debug, Clone)]
pub struct RealityPredictionBuilder {
    prediction_id: [u8; 16],
    task_id: TaskId,
    session_id: [u8; 16],
    language: Language,
    predicted_oracle_pass: f32,
    predicted_test_pass: Vec<f32>,
    predicted_runtime_trace: [f32; 32],
    ood_score: f32,
    outcome_set: ConformalSet,
    calibrated_confidence: f32,
    degraded_status: bool,
    granger_attestations: BTreeMap<String, f32>,
    source_panel_sha: [u8; 32],
    calibration_version: String,
    created_at_unix_ms: i64,
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
            task_id,
            session_id,
            language,
            predicted_oracle_pass: 0.0,
            predicted_test_pass: Vec::new(),
            predicted_runtime_trace: [0.0; 32],
            ood_score: 0.0,
            outcome_set,
            calibrated_confidence: 0.0,
            degraded_status: false,
            granger_attestations: BTreeMap::new(),
            source_panel_sha: [0u8; 32],
            calibration_version: String::new(),
            created_at_unix_ms: chrono::Utc::now().timestamp_millis(),
        }
    }

    pub fn prediction_id(mut self, prediction_id: [u8; 16]) -> Self {
        self.prediction_id = prediction_id;
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

    pub fn source_panel_sha(mut self, value: [u8; 32]) -> Self {
        self.source_panel_sha = value;
        self
    }

    pub fn calibration_version(mut self, value: impl Into<String>) -> Self {
        self.calibration_version = value.into();
        self
    }

    pub fn build(self) -> Result<RealityPrediction, MejepaInferError> {
        RealityPrediction::try_new(RealityPrediction {
            prediction_id: self.prediction_id,
            task_id: self.task_id,
            session_id: self.session_id,
            language: self.language,
            predicted_oracle_pass: self.predicted_oracle_pass,
            predicted_test_pass: self.predicted_test_pass,
            predicted_runtime_trace: self.predicted_runtime_trace,
            ood_score: self.ood_score,
            outcome_set: self.outcome_set,
            calibrated_confidence: self.calibrated_confidence,
            degraded_status: self.degraded_status,
            granger_attestations: self.granger_attestations,
            source_panel_sha: self.source_panel_sha,
            calibration_version: self.calibration_version,
            created_at_unix_ms: self.created_at_unix_ms,
        })
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
    Exception,
    TypeError,
    NameError,
    Timeout,
    Crash,
    CompileError,
    Flaky,
    NoTestsCollected,
    Other,
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

pub(crate) fn validate_probability(field: &str, value: f32) -> Result<(), MejepaInferError> {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return Err(MejepaInferError::NanDetected {
            nan_source: field.to_string(),
            detail: format!("{field} must be finite and in [0, 1]; got {value}"),
        });
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
