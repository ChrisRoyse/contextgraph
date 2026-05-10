use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::MejepaInferError;

const MAX_ID_BYTES: usize = 512;
const MAX_TESTS: usize = 1024;
const MAX_CHUNKS: usize = 4096;
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
    if value.trim().is_empty() {
        return Err(MejepaInferError::InvalidInput {
            field: field.to_string(),
            detail: "must be non-empty".to_string(),
        });
    }
    if value.len() > MAX_ID_BYTES {
        return Err(MejepaInferError::InvalidInput {
            field: field.to_string(),
            detail: format!("exceeds {MAX_ID_BYTES} bytes"),
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
