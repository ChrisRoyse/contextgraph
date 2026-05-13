use super::error::{EvalError, EvalErrorCode};
use crate::types::{Language, OracleOutcome, PatchBundle, RealityPrediction, TaskContext, TaskId};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MutationCategory {
    KnownGood,
    SubtleFlip,
    OffByOne,
    SwapVariable,
    DeleteTestCall,
    WrongFile,
    OverEngineer,
    CompileError,
}

impl MutationCategory {
    pub fn all() -> [Self; 8] {
        [
            Self::KnownGood,
            Self::SubtleFlip,
            Self::OffByOne,
            Self::SwapVariable,
            Self::DeleteTestCall,
            Self::WrongFile,
            Self::OverEngineer,
            Self::CompileError,
        ]
    }

    pub fn slug(self) -> &'static str {
        match self {
            Self::KnownGood => "known_good",
            Self::SubtleFlip => "subtle_flip",
            Self::OffByOne => "off_by_one",
            Self::SwapVariable => "swap_variable",
            Self::DeleteTestCall => "delete_test_call",
            Self::WrongFile => "wrong_file",
            Self::OverEngineer => "over_engineer",
            Self::CompileError => "compile_error",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvalConfig {
    pub rolling_window_size: usize,
    pub min_samples_per_slice: usize,
    pub conformal_expected_coverage: f32,
    pub conformal_band: f32,
    pub ood_auc_min: f32,
    pub gtau_pass_rate_min: f32,
    pub correlation_min: f32,
    pub regression_max_drop: f32,
    pub q1_pass_rate_min: f32,
    pub q3_side_effect_min: f32,
}

impl Default for EvalConfig {
    fn default() -> Self {
        Self {
            rolling_window_size: 100,
            min_samples_per_slice: 2,
            conformal_expected_coverage: 0.90,
            conformal_band: 0.05,
            ood_auc_min: 0.85,
            gtau_pass_rate_min: 0.95,
            correlation_min: 0.25,
            regression_max_drop: 0.02,
            q1_pass_rate_min: 0.99,
            q3_side_effect_min: 0.90,
        }
    }
}

impl EvalConfig {
    pub fn validate(&self) -> Result<(), EvalError> {
        if self.rolling_window_size == 0 {
            return Err(EvalError::new(
                EvalErrorCode::InvalidConfig,
                "rolling_window_size must be greater than zero",
            ));
        }
        if self.min_samples_per_slice == 0 {
            return Err(EvalError::new(
                EvalErrorCode::InvalidConfig,
                "min_samples_per_slice must be greater than zero",
            ));
        }
        for (name, value) in [
            (
                "conformal_expected_coverage",
                self.conformal_expected_coverage,
            ),
            ("conformal_band", self.conformal_band),
            ("ood_auc_min", self.ood_auc_min),
            ("gtau_pass_rate_min", self.gtau_pass_rate_min),
            ("correlation_min", self.correlation_min),
            ("regression_max_drop", self.regression_max_drop),
            ("q1_pass_rate_min", self.q1_pass_rate_min),
            ("q3_side_effect_min", self.q3_side_effect_min),
        ] {
            validate_unit(name, value)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HoldoutPanel {
    pub task_id: TaskId,
    pub mutation_category: MutationCategory,
    pub language: Language,
    pub patch: PatchBundle,
    pub context: TaskContext,
    pub actual_oracle: OracleOutcome,
    pub panel_sha: [u8; 32],
}

impl HoldoutPanel {
    pub fn validate(&self) -> Result<(), EvalError> {
        self.task_id
            .validate("holdout.task_id")
            .map_err(EvalError::from)?;
        self.patch.validate().map_err(EvalError::from)?;
        self.context.validate().map_err(EvalError::from)?;
        if self.context.task_id != self.task_id {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                format!(
                    "holdout context task_id {} does not match panel task_id {}",
                    self.context.task_id.0, self.task_id.0
                ),
            ));
        }
        if self.context.language != self.language {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                "holdout context language does not match panel language",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvalObservation {
    pub task_id: TaskId,
    pub mutation_category: MutationCategory,
    pub language: Language,
    pub actual_oracle: OracleOutcome,
    pub prediction: RealityPrediction,
    pub gtau_passed: bool,
    pub approved: bool,
    pub live_prediction_readback: bool,
    pub latency_ms: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConformalHealthEntry {
    pub expected_coverage: f32,
    pub empirical_coverage: f32,
    pub sample_count: usize,
    pub within_band: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActiveLearningSummary {
    pub queued_count: usize,
    pub evicted_count: usize,
    pub ood_escalation_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StateTransferDiagnostic {
    pub wasserstein_1: f32,
    pub transfer_score: f32,
    pub performance_deploy: f32,
}

impl StateTransferDiagnostic {
    pub fn validate(&self, name: &str) -> Result<(), EvalError> {
        validate_unit(&format!("{name}.wasserstein_1"), self.wasserstein_1)?;
        validate_unit(&format!("{name}.transfer_score"), self.transfer_score)?;
        validate_unit(
            &format!("{name}.performance_deploy"),
            self.performance_deploy,
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuxHeadDistillationSummary {
    pub teacher_report_hash: String,
    pub student_report_hash: String,
    pub max_delta: f32,
    pub accepted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegressionCheck {
    pub name: String,
    pub previous: f32,
    pub current: f32,
    pub drop: f32,
    pub passed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpenResearchQuestionStatus {
    pub id: String,
    pub question: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvalProvenance {
    pub corpus_sha: String,
    pub eval_code_version: String,
    pub calibration_version: String,
    pub generated_by: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvalReport {
    pub report_date: String,
    pub generated_at_unix_ms: i64,
    pub rolling_window_size: usize,
    pub holdout_count: usize,
    pub overall_correlation: Option<f32>,
    pub per_category_correlation: BTreeMap<MutationCategory, Option<f32>>,
    pub per_language_correlation: BTreeMap<Language, Option<f32>>,
    pub per_cell_correlation: BTreeMap<String, Option<f32>>,
    pub bayesian_shrinkage: BTreeMap<String, f32>,
    pub conformal_coverage_health: BTreeMap<Language, ConformalHealthEntry>,
    pub ood_calibration_health: BTreeMap<Language, Option<f32>>,
    pub gtau_pass_rate: BTreeMap<Language, f32>,
    pub active_learning: ActiveLearningSummary,
    pub state_transfer_diagnostic: Option<StateTransferDiagnostic>,
    pub per_cell_state_transfer: BTreeMap<String, Option<StateTransferDiagnostic>>,
    pub aux_head_distillation: Option<AuxHeadDistillationSummary>,
    pub regression_checks: Vec<RegressionCheck>,
    pub open_research_questions: Vec<OpenResearchQuestionStatus>,
    pub q1_pass_rate: f32,
    pub q2_report_correlation: Option<f32>,
    pub q3_side_effect_agreement: Option<f32>,
    pub ship_gate_passed: bool,
    pub ship_gate_failures: Vec<String>,
    pub provenance: EvalProvenance,
    pub wall_clock_seconds: f32,
}

impl EvalReport {
    pub fn validate(&self) -> Result<(), EvalError> {
        if self.report_date.trim().is_empty() {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                "report_date must be non-empty",
            ));
        }
        if self.holdout_count == 0 {
            return Err(EvalError::new(
                EvalErrorCode::EmptyHoldout,
                "holdout_count must be greater than zero",
            ));
        }
        validate_optional_correlation("overall_correlation", self.overall_correlation)?;
        validate_unit("q1_pass_rate", self.q1_pass_rate)?;
        validate_optional_correlation("q2_report_correlation", self.q2_report_correlation)?;
        validate_optional_unit("q3_side_effect_agreement", self.q3_side_effect_agreement)?;
        if !self.wall_clock_seconds.is_finite() || self.wall_clock_seconds < 0.0 {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                format!("wall_clock_seconds invalid: {}", self.wall_clock_seconds),
            ));
        }
        for (category, value) in &self.per_category_correlation {
            validate_optional_correlation(
                &format!("per_category_correlation.{category:?}"),
                *value,
            )?;
        }
        for (language, value) in &self.per_language_correlation {
            validate_optional_correlation(
                &format!("per_language_correlation.{language:?}"),
                *value,
            )?;
        }
        for (cell, value) in &self.per_cell_correlation {
            validate_optional_correlation(&format!("per_cell_correlation.{cell}"), *value)?;
        }
        for (key, value) in &self.bayesian_shrinkage {
            validate_unit(&format!("bayesian_shrinkage.{key}"), *value)?;
        }
        for health in self.conformal_coverage_health.values() {
            validate_unit("conformal.expected_coverage", health.expected_coverage)?;
            validate_unit("conformal.empirical_coverage", health.empirical_coverage)?;
        }
        for value in self.ood_calibration_health.values() {
            validate_optional_unit("ood_auc", *value)?;
        }
        for value in self.gtau_pass_rate.values() {
            validate_unit("gtau_pass_rate", *value)?;
        }
        if let Some(diagnostic) = &self.state_transfer_diagnostic {
            diagnostic.validate("state_transfer_diagnostic")?;
        }
        for (cell, diagnostic) in &self.per_cell_state_transfer {
            if !self.per_cell_correlation.contains_key(cell) {
                return Err(EvalError::new(
                    EvalErrorCode::InvalidInput,
                    format!("per_cell_state_transfer.{cell} has no per_cell_correlation entry"),
                ));
            }
            if let Some(diagnostic) = diagnostic {
                diagnostic.validate(&format!("per_cell_state_transfer.{cell}"))?;
            }
        }
        Ok(())
    }

    pub fn canonical_json(&self) -> Result<Vec<u8>, EvalError> {
        let mut clone = self.clone();
        clone.generated_at_unix_ms = 0;
        clone.wall_clock_seconds = 0.0;
        Ok(serde_json::to_vec(&clone)?)
    }

    pub fn determinism_hash(&self) -> Result<String, EvalError> {
        let mut hasher = Sha256::new();
        hasher.update(self.canonical_json()?);
        Ok(hex::encode(hasher.finalize()))
    }
}

pub fn language_slug(language: Language) -> &'static str {
    match language {
        Language::Rust => "rust",
        Language::Python => "python",
        Language::Javascript => "javascript",
        Language::Typescript => "typescript",
        Language::Go => "go",
        Language::Java => "java",
        Language::C => "c",
        Language::Cpp => "cpp",
        Language::CSharp => "c_sharp",
        Language::Ruby => "ruby",
        Language::Php => "php",
    }
}

pub fn cell_key(category: MutationCategory, language: Language) -> String {
    format!("{}::{}", category.slug(), language_slug(language))
}

pub(crate) fn validate_unit(name: &str, value: f32) -> Result<(), EvalError> {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return Err(EvalError::new(
            EvalErrorCode::InvalidInput,
            format!("{name} must be finite and in [0,1]; got {value}"),
        ));
    }
    Ok(())
}

pub(crate) fn validate_optional_unit(name: &str, value: Option<f32>) -> Result<(), EvalError> {
    if let Some(value) = value {
        validate_unit(name, value)?;
    }
    Ok(())
}

pub(crate) fn validate_correlation(name: &str, value: f32) -> Result<(), EvalError> {
    if !value.is_finite() || !(-1.0..=1.0).contains(&value) {
        return Err(EvalError::new(
            EvalErrorCode::InvalidInput,
            format!("{name} must be finite and in [-1,1]; got {value}"),
        ));
    }
    Ok(())
}

pub(crate) fn validate_optional_correlation(
    name: &str,
    value: Option<f32>,
) -> Result<(), EvalError> {
    if let Some(value) = value {
        validate_correlation(name, value)?;
    }
    Ok(())
}
