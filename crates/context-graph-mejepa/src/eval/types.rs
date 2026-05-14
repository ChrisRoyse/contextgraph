use super::cell_exemptions::CellExemption;
use super::convergence_tracker::CellConvergenceEta;
use super::error::{EvalError, EvalErrorCode};
use super::per_head_calibration_tracker::PredictionClassCalibration;
use crate::heal::drift_attribution::{FailingCellClassification, FailingCellRootCause};
use crate::types::{
    FailureModeClass, Language, OracleOutcome, PatchBundle, RealityPrediction, TaskContext, TaskId,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

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
    pub convergence_target_correlation: f32,
    pub convergence_history_windows: usize,
    pub convergence_eta_min_points: usize,
    pub conformal_expected_coverage: f32,
    pub conformal_band: f32,
    pub ood_auc_min: f32,
    pub gtau_pass_rate_min: f32,
    pub correlation_min: f32,
    pub regression_max_drop: f32,
    pub q1_pass_rate_min: f32,
    pub q3_side_effect_min: f32,
    pub failure_mode_precision_min: f32,
    pub failure_mode_recall_min: f32,
}

impl Default for EvalConfig {
    fn default() -> Self {
        Self {
            rolling_window_size: 100,
            min_samples_per_slice: 2,
            convergence_target_correlation: 0.95,
            convergence_history_windows: 8,
            convergence_eta_min_points: 3,
            conformal_expected_coverage: 0.90,
            conformal_band: 0.05,
            ood_auc_min: 0.85,
            gtau_pass_rate_min: 0.95,
            correlation_min: 0.25,
            regression_max_drop: 0.02,
            q1_pass_rate_min: 0.99,
            q3_side_effect_min: 0.90,
            failure_mode_precision_min: 0.90,
            failure_mode_recall_min: 0.70,
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
        if self.convergence_history_windows == 0 {
            return Err(EvalError::new(
                EvalErrorCode::InvalidConfig,
                "convergence_history_windows must be greater than zero",
            ));
        }
        if self.convergence_eta_min_points < 2 {
            return Err(EvalError::new(
                EvalErrorCode::InvalidConfig,
                "convergence_eta_min_points must be at least 2",
            ));
        }
        if self.convergence_eta_min_points > self.convergence_history_windows {
            return Err(EvalError::new(
                EvalErrorCode::InvalidConfig,
                "convergence_eta_min_points cannot exceed convergence_history_windows",
            ));
        }
        validate_correlation(
            "convergence_target_correlation",
            self.convergence_target_correlation,
        )?;
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
            (
                "failure_mode_precision_min",
                self.failure_mode_precision_min,
            ),
            ("failure_mode_recall_min", self.failure_mode_recall_min),
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
    pub actual_failure_modes: Vec<FailureModeClass>,
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
        validate_actual_failure_modes("holdout.actual_failure_modes", &self.actual_failure_modes)?;
        if self.actual_oracle == OracleOutcome::Pass && !self.actual_failure_modes.is_empty() {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                "passing holdout panel cannot carry actual failure modes",
            ));
        }
        if self.actual_oracle == OracleOutcome::Fail && self.actual_failure_modes.is_empty() {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                "failing holdout panel must carry at least one actual failure mode",
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
    pub actual_failure_modes: Vec<FailureModeClass>,
    pub prediction: RealityPrediction,
    pub gtau_passed: bool,
    pub approved: bool,
    pub live_prediction_readback: bool,
    pub latency_ms: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FailureModeClassMetrics {
    pub failure_class: FailureModeClass,
    pub sample_count: usize,
    pub actual_positive_count: usize,
    pub predicted_positive_count: usize,
    pub true_positive: usize,
    pub false_positive: usize,
    pub false_negative: usize,
    pub true_negative: usize,
    pub precision: f32,
    pub recall: f32,
    pub f1: f32,
    pub precision_threshold: f32,
    pub recall_threshold: f32,
    pub passed_threshold: bool,
    pub weakness: Option<String>,
}

impl FailureModeClassMetrics {
    pub fn validate(&self, expected_class: FailureModeClass) -> Result<(), EvalError> {
        if self.failure_class != expected_class {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                format!(
                    "per_failure_mode_class key {} does not match payload {}",
                    expected_class.slug(),
                    self.failure_class.slug()
                ),
            ));
        }
        if self.sample_count
            != self.true_positive + self.false_positive + self.false_negative + self.true_negative
        {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                format!(
                    "per_failure_mode_class.{} count partition mismatch",
                    expected_class.slug()
                ),
            ));
        }
        if self.actual_positive_count != self.true_positive + self.false_negative {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                format!(
                    "per_failure_mode_class.{} actual_positive_count mismatch",
                    expected_class.slug()
                ),
            ));
        }
        if self.predicted_positive_count != self.true_positive + self.false_positive {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                format!(
                    "per_failure_mode_class.{} predicted_positive_count mismatch",
                    expected_class.slug()
                ),
            ));
        }
        validate_unit(
            &format!("per_failure_mode_class.{}.precision", expected_class.slug()),
            self.precision,
        )?;
        validate_unit(
            &format!("per_failure_mode_class.{}.recall", expected_class.slug()),
            self.recall,
        )?;
        validate_unit(
            &format!("per_failure_mode_class.{}.f1", expected_class.slug()),
            self.f1,
        )?;
        validate_unit(
            &format!(
                "per_failure_mode_class.{}.precision_threshold",
                expected_class.slug()
            ),
            self.precision_threshold,
        )?;
        validate_unit(
            &format!(
                "per_failure_mode_class.{}.recall_threshold",
                expected_class.slug()
            ),
            self.recall_threshold,
        )?;
        let threshold_result =
            self.precision >= self.precision_threshold && self.recall >= self.recall_threshold;
        if self.passed_threshold != threshold_result {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                format!(
                    "per_failure_mode_class.{} passed_threshold mismatch",
                    expected_class.slug()
                ),
            ));
        }
        if self.passed_threshold && self.weakness.is_some() {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                format!(
                    "per_failure_mode_class.{} passed class cannot report weakness",
                    expected_class.slug()
                ),
            ));
        }
        if !self.passed_threshold
            && self
                .weakness
                .as_ref()
                .is_none_or(|value| value.trim().is_empty())
        {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                format!(
                    "per_failure_mode_class.{} failing class requires weakness",
                    expected_class.slug()
                ),
            ));
        }
        Ok(())
    }
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
    pub cell_exemptions: BTreeMap<String, CellExemption>,
    pub bayesian_shrinkage: BTreeMap<String, f32>,
    pub conformal_coverage_health: BTreeMap<Language, ConformalHealthEntry>,
    pub ood_calibration_health: BTreeMap<Language, Option<f32>>,
    pub gtau_pass_rate: BTreeMap<Language, f32>,
    pub per_prediction_class_calibration: BTreeMap<String, PredictionClassCalibration>,
    pub per_failure_mode_class: BTreeMap<FailureModeClass, FailureModeClassMetrics>,
    pub per_cell_convergence_eta: BTreeMap<String, CellConvergenceEta>,
    pub active_learning: ActiveLearningSummary,
    pub state_transfer_diagnostic: Option<StateTransferDiagnostic>,
    pub per_cell_state_transfer: BTreeMap<String, Option<StateTransferDiagnostic>>,
    pub failing_cell_classifications: BTreeMap<String, FailingCellClassification>,
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
        for (cell, exemption) in &self.cell_exemptions {
            if cell != &exemption.cell {
                return Err(EvalError::new(
                    EvalErrorCode::InvalidInput,
                    format!("cell_exemptions key {cell} does not match payload cell"),
                ));
            }
            exemption.validate()?;
        }
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
        for (class_name, calibration) in &self.per_prediction_class_calibration {
            if class_name != &calibration.class_name {
                return Err(EvalError::new(
                    EvalErrorCode::InvalidInput,
                    format!(
                        "per_prediction_class_calibration key {class_name} does not match class_name {}",
                        calibration.class_name
                    ),
                ));
            }
            calibration.validate(&format!("per_prediction_class_calibration.{class_name}"))?;
        }
        for (failure_class, metrics) in &self.per_failure_mode_class {
            metrics.validate(*failure_class)?;
            if metrics.sample_count != self.holdout_count {
                return Err(EvalError::new(
                    EvalErrorCode::InvalidInput,
                    format!(
                        "per_failure_mode_class.{} sample_count {} does not match holdout_count {}",
                        failure_class.slug(),
                        metrics.sample_count,
                        self.holdout_count
                    ),
                ));
            }
        }
        for failure_class in FailureModeClass::all() {
            if !self.per_failure_mode_class.contains_key(&failure_class) {
                return Err(EvalError::new(
                    EvalErrorCode::InvalidInput,
                    format!(
                        "per_failure_mode_class missing required class {}",
                        failure_class.slug()
                    ),
                ));
            }
        }
        for (cell, eta) in &self.per_cell_convergence_eta {
            if !self.per_cell_correlation.contains_key(cell) {
                return Err(EvalError::new(
                    EvalErrorCode::InvalidInput,
                    format!("per_cell_convergence_eta.{cell} has no per_cell_correlation entry"),
                ));
            }
            eta.validate(cell)?;
        }
        for cell in self.per_cell_correlation.keys() {
            if !self.per_cell_convergence_eta.contains_key(cell) {
                return Err(EvalError::new(
                    EvalErrorCode::InvalidInput,
                    format!("per_cell_convergence_eta missing required cell {cell}"),
                ));
            }
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
        for (cell, classification) in &self.failing_cell_classifications {
            if !self.per_cell_correlation.contains_key(cell) {
                return Err(EvalError::new(
                    EvalErrorCode::InvalidInput,
                    format!(
                        "failing_cell_classifications.{cell} has no per_cell_correlation entry"
                    ),
                ));
            }
            if classification.cell != *cell {
                return Err(EvalError::new(
                    EvalErrorCode::InvalidInput,
                    format!(
                        "failing_cell_classifications key {cell} does not match payload cell {}",
                        classification.cell
                    ),
                ));
            }
            validate_unit(
                &format!("failing_cell_classifications.{cell}.confidence"),
                classification.confidence,
            )?;
            if classification.heuristic.trim().is_empty() || classification.evidence.is_empty() {
                return Err(EvalError::new(
                    EvalErrorCode::InvalidInput,
                    format!("failing_cell_classifications.{cell} requires heuristic and evidence"),
                ));
            }
            if matches!(classification.root_cause, FailingCellRootCause::Unknown)
                && classification.confidence > 0.50
            {
                return Err(EvalError::new(
                    EvalErrorCode::InvalidInput,
                    format!(
                        "failing_cell_classifications.{cell}.unknown_confidence must be <= 0.50"
                    ),
                ));
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

pub const ACTIVE_PYTHON_SHIP_GATE_NAME: &str = "python_swebench_lite_300x8";
pub const ACTIVE_PYTHON_SHIP_GATE_GRID: &str = "8 mutation categories x python";
pub const ACTIVE_PYTHON_SHIP_GATE_CELL_COUNT: usize = 8;

pub fn required_active_python_ship_gate_cells() -> Vec<(MutationCategory, Language, String)> {
    MutationCategory::all()
        .into_iter()
        .map(|category| {
            (
                category,
                Language::Python,
                cell_key(category, Language::Python),
            )
        })
        .collect()
}

pub fn validate_active_python_ship_gate_report(report: &EvalReport) -> Result<(), EvalError> {
    let expected_cells = active_python_ship_gate_cell_set();
    let actual_cells = report
        .per_cell_correlation
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>();
    if actual_cells != expected_cells {
        let missing = expected_cells
            .difference(&actual_cells)
            .cloned()
            .collect::<Vec<_>>();
        let extra = actual_cells
            .difference(&expected_cells)
            .cloned()
            .collect::<Vec<_>>();
        let non_python = actual_cells
            .iter()
            .filter(|cell| !cell.ends_with("::python"))
            .cloned()
            .collect::<Vec<_>>();
        let code = if non_python.is_empty() {
            "MEJEPA_ACTIVE_PYTHON_GATE_CELL_SET_MISMATCH"
        } else {
            "MEJEPA_ACTIVE_PYTHON_GATE_STALE_GRID"
        };
        return Err(EvalError::new(
            EvalErrorCode::InvalidInput,
            format!(
                "{code}: expected {} cells for {ACTIVE_PYTHON_SHIP_GATE_GRID}; observed={} missing={} extra={} non_python={}",
                ACTIVE_PYTHON_SHIP_GATE_CELL_COUNT,
                actual_cells.len(),
                preview_strings(&missing),
                preview_strings(&extra),
                preview_strings(&non_python)
            ),
        ));
    }

    validate_cell_key_subset(
        "cell_exemptions",
        report.cell_exemptions.keys(),
        &expected_cells,
    )?;
    validate_cell_key_subset(
        "bayesian_shrinkage",
        report.bayesian_shrinkage.keys(),
        &expected_cells,
    )?;
    validate_cell_key_subset(
        "per_cell_state_transfer",
        report.per_cell_state_transfer.keys(),
        &expected_cells,
    )?;
    validate_cell_key_subset(
        "failing_cell_classifications",
        report.failing_cell_classifications.keys(),
        &expected_cells,
    )?;

    validate_python_only_languages(
        "per_language_correlation",
        report.per_language_correlation.keys().copied(),
    )?;
    validate_python_only_languages(
        "conformal_coverage_health",
        report.conformal_coverage_health.keys().copied(),
    )?;
    validate_python_only_languages(
        "ood_calibration_health",
        report.ood_calibration_health.keys().copied(),
    )?;
    validate_python_only_languages("gtau_pass_rate", report.gtau_pass_rate.keys().copied())?;

    Ok(())
}

fn active_python_ship_gate_cell_set() -> BTreeSet<String> {
    required_active_python_ship_gate_cells()
        .into_iter()
        .map(|(_, _, cell)| cell)
        .collect()
}

fn validate_cell_key_subset<'a, I>(
    field: &str,
    keys: I,
    expected_cells: &BTreeSet<String>,
) -> Result<(), EvalError>
where
    I: IntoIterator<Item = &'a String>,
{
    let invalid = keys
        .into_iter()
        .filter(|cell| !expected_cells.contains(*cell))
        .cloned()
        .collect::<Vec<_>>();
    if invalid.is_empty() {
        return Ok(());
    }
    Err(EvalError::new(
        EvalErrorCode::InvalidInput,
        format!(
            "MEJEPA_ACTIVE_PYTHON_GATE_STALE_CELL_METADATA: {field} contains cells outside {ACTIVE_PYTHON_SHIP_GATE_GRID}: {}",
            preview_strings(&invalid)
        ),
    ))
}

fn validate_python_only_languages<I>(field: &str, languages: I) -> Result<(), EvalError>
where
    I: IntoIterator<Item = Language>,
{
    let invalid = languages
        .into_iter()
        .filter(|language| *language != Language::Python)
        .map(|language| language_slug(language).to_string())
        .collect::<Vec<_>>();
    if invalid.is_empty() {
        return Ok(());
    }
    Err(EvalError::new(
        EvalErrorCode::InvalidInput,
        format!(
            "MEJEPA_ACTIVE_PYTHON_GATE_STALE_LANGUAGE_METADATA: {field} contains non-Python languages for {ACTIVE_PYTHON_SHIP_GATE_GRID}: {}",
            preview_strings(&invalid)
        ),
    ))
}

fn preview_strings(values: &[String]) -> String {
    const MAX_PREVIEW: usize = 8;
    if values.is_empty() {
        return "[]".to_string();
    }
    let mut preview = values.iter().take(MAX_PREVIEW).cloned().collect::<Vec<_>>();
    if values.len() > MAX_PREVIEW {
        preview.push(format!("...{} more", values.len() - MAX_PREVIEW));
    }
    format!("[{}]", preview.join(", "))
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

fn validate_actual_failure_modes(name: &str, modes: &[FailureModeClass]) -> Result<(), EvalError> {
    const MAX_ACTUAL_FAILURE_MODES: usize = 8;
    if modes.len() > MAX_ACTUAL_FAILURE_MODES {
        return Err(EvalError::new(
            EvalErrorCode::InvalidInput,
            format!(
                "{name} exceeds max failure-mode labels {MAX_ACTUAL_FAILURE_MODES}; got {}",
                modes.len()
            ),
        ));
    }
    let mut seen = BTreeSet::new();
    for mode in modes {
        if !seen.insert(*mode) {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                format!("{name} contains duplicate failure mode {}", mode.slug()),
            ));
        }
    }
    Ok(())
}
