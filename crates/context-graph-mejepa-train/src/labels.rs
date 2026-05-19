use std::path::PathBuf;
use std::process::Command;

use context_graph_mejepa::label_transfer_audit::{
    apply_and_persist_label_transfer_decision, LabelTransferApplicationInput,
    LabelTransferAuditRecord,
};
use context_graph_mejepa::{
    AccuracyMetric, ChunkId, CostAxis, EdgeCaseClass, FailureModeClass, Language, LatentBugClass,
    PerfAxis, PhaseBPredictionSurfaces, PredictedAccuracyDegradation, PredictedCostRegression,
    PredictedPerfRegression, PredictionId, ReasoningClass, RootCauseClass, SecurityConcernClass,
    TestDeltaKind, TestId, TestOutcome,
};
use context_graph_mejepa_instruments::{
    Q4AccuracyLabel, Q4AccuracyMetricKind, Q4CostKind, Q4CostLabel, Q4CostLabelKind,
    Q4PerfCategory, Q4PerfLabel, Q4ReasoningClass, Q4ReasoningLabel, Q4SecurityClass,
    Q4SecurityLabel,
};
use rocksdb::DB;
use serde::{Deserialize, Serialize};

use crate::learning_signal::{non_empty, UtmlError, UtmlErrorCode};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LabelSource {
    Pytest,
    PythonUnittest,
    CargoTest,
    CargoClippy,
    PythonSecurityAnalyzer,
    PythonPerfAnalyzer,
    PythonAccuracyAnalyzer,
    AgentResponse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LabelInputLanguage {
    Python,
    Rust,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalyzerOutputMode {
    CommandLog,
    StrictJson,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PhaseBLabelAnalyzerConfig {
    pub source: LabelSource,
    pub language: LabelInputLanguage,
    pub program: PathBuf,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub output_mode: AnalyzerOutputMode,
    pub require_labels: bool,
    pub max_output_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandEvidence {
    pub source: LabelSource,
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FailureModeLabel {
    pub test_id: Option<TestId>,
    pub failure_class: FailureModeClass,
    pub root_cause_class: RootCauseClass,
    pub evidence_line: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TestOutcomeLabel {
    pub test_id: TestId,
    pub current_outcome: TestOutcome,
    pub predicted_outcome: TestOutcome,
    pub delta_kind: TestDeltaKind,
    pub evidence_line: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EdgeCaseLabel {
    pub edge_class: EdgeCaseClass,
    pub evidence_line: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LatentBugLabel {
    pub bug_class: LatentBugClass,
    pub evidence_line: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SecurityLabel {
    pub class: SecurityConcernClass,
    pub evidence_line: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExtractedPhaseBLabels {
    pub source: LabelSource,
    pub failure_modes: Vec<FailureModeLabel>,
    pub failed_tests: Vec<TestOutcomeLabel>,
    pub edge_cases: Vec<EdgeCaseLabel>,
    pub latent_bugs: Vec<LatentBugLabel>,
    pub security_concerns: Vec<SecurityLabel>,
    pub reasoning_class: ReasoningClass,
    pub touched_files: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LabelTransferApplicationContext {
    pub prediction_id: PredictionId,
    pub label_source_id: String,
    pub source_language: Language,
    pub target_language: Language,
    pub source_chunk_id: ChunkId,
    pub target_chunk_id: ChunkId,
    pub target_root_cause: RootCauseClass,
    pub similarity: f32,
    pub source_cross_language_correlation: f32,
    pub created_at_unix_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppliedFailureModeLabel {
    pub label: FailureModeLabel,
    pub decision: LabelTransferAuditRecord,
    pub effective_weight: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RejectedFailureModeLabel {
    pub label: FailureModeLabel,
    pub decision: LabelTransferAuditRecord,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppliedPhaseBLabels {
    pub source: LabelSource,
    pub accepted_failure_modes: Vec<AppliedFailureModeLabel>,
    pub rejected_failure_modes: Vec<RejectedFailureModeLabel>,
    pub transfer_decisions: Vec<LabelTransferAuditRecord>,
}

impl CommandEvidence {
    pub fn combined_output(&self) -> String {
        match (self.stdout.is_empty(), self.stderr.is_empty()) {
            (false, false) => format!("{}\n{}", self.stdout, self.stderr),
            (false, true) => self.stdout.clone(),
            (true, false) => self.stderr.clone(),
            (true, true) => String::new(),
        }
    }
}

pub fn run_phase_b_label_analyzer(
    config: &PhaseBLabelAnalyzerConfig,
) -> Result<ExtractedPhaseBLabels, UtmlError> {
    validate_analyzer_config(config)?;
    let output = Command::new(&config.program)
        .args(&config.args)
        .current_dir(&config.cwd)
        .output()
        .map_err(|err| {
            UtmlError::new(
                UtmlErrorCode::Io,
                format!(
                    "phase-b label analyzer spawn failed: program={} cwd={} err={err}",
                    config.program.display(),
                    config.cwd.display()
                ),
            )
        })?;
    let stdout = bounded_output("stdout", &output.stdout, config.max_output_bytes)?;
    let stderr = bounded_output("stderr", &output.stderr, config.max_output_bytes)?;
    let exit_code = output.status.code().unwrap_or(-1);
    let labels = match config.output_mode {
        AnalyzerOutputMode::CommandLog => extract_phase_b_labels(&CommandEvidence {
            source: config.source.clone(),
            exit_code,
            stdout,
            stderr,
        })?,
        AnalyzerOutputMode::StrictJson => {
            let labels: ExtractedPhaseBLabels = serde_json::from_str(&stdout).map_err(|err| {
                UtmlError::new(
                    UtmlErrorCode::InvalidSignal,
                    format!("strict analyzer JSON parse failed: {err}"),
                )
            })?;
            validate_extracted_labels(&labels)?;
            labels
        }
    };
    if labels.source != config.source {
        return Err(UtmlError::new(
            UtmlErrorCode::InvalidSignal,
            format!(
                "analyzer label source mismatch: expected {:?}, got {:?}",
                config.source, labels.source
            ),
        ));
    }
    if config.require_labels {
        require_non_empty_labels(&labels)?;
    }
    validate_extracted_labels(&labels)?;
    Ok(labels)
}

pub fn extract_phase_b_labels(
    evidence: &CommandEvidence,
) -> Result<ExtractedPhaseBLabels, UtmlError> {
    let combined = evidence.combined_output();
    non_empty("label_extraction.command_output", combined.as_bytes())?;
    let mut labels = ExtractedPhaseBLabels {
        source: evidence.source.clone(),
        failure_modes: Vec::new(),
        failed_tests: Vec::new(),
        edge_cases: Vec::new(),
        latent_bugs: Vec::new(),
        security_concerns: Vec::new(),
        reasoning_class: if evidence.exit_code == 0 {
            ReasoningClass::Correct
        } else {
            ReasoningClass::PlausibleButWrong
        },
        touched_files: Vec::new(),
    };

    for line in combined
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        if let Some(test_id) = parse_failed_test_id(line, &evidence.source) {
            labels.failed_tests.push(TestOutcomeLabel {
                test_id,
                current_outcome: TestOutcome::Pass,
                predicted_outcome: TestOutcome::Fail,
                delta_kind: TestDeltaKind::NewFailure,
                evidence_line: line.to_string(),
            });
        }
        if let Some(failure_class) = classify_failure_line(line) {
            labels.failure_modes.push(FailureModeLabel {
                test_id: parse_failed_test_id(line, &evidence.source),
                failure_class,
                root_cause_class: root_cause_for_failure(failure_class),
                evidence_line: line.to_string(),
            });
        }
        if let Some(edge_class) = classify_edge_case_line(line) {
            labels.edge_cases.push(EdgeCaseLabel {
                edge_class,
                evidence_line: line.to_string(),
            });
        }
        if let Some(bug_class) = classify_latent_bug_line(line) {
            labels.latent_bugs.push(LatentBugLabel {
                bug_class,
                evidence_line: line.to_string(),
            });
        }
        if let Some(class) = classify_security_line(line) {
            labels.security_concerns.push(SecurityLabel {
                class,
                evidence_line: line.to_string(),
            });
        }
        if let Some(path) = parse_file_path(line) {
            if !labels.touched_files.contains(&path) {
                labels.touched_files.push(path);
            }
        }
    }

    Ok(labels)
}

pub fn extract_pytest_labels(
    stdout: &str,
    stderr: &str,
    exit_code: i32,
) -> Result<ExtractedPhaseBLabels, UtmlError> {
    extract_phase_b_labels(&CommandEvidence {
        source: LabelSource::Pytest,
        exit_code,
        stdout: stdout.to_string(),
        stderr: stderr.to_string(),
    })
}

pub fn extract_cargo_test_labels(
    stdout: &str,
    stderr: &str,
    exit_code: i32,
) -> Result<ExtractedPhaseBLabels, UtmlError> {
    extract_phase_b_labels(&CommandEvidence {
        source: LabelSource::CargoTest,
        exit_code,
        stdout: stdout.to_string(),
        stderr: stderr.to_string(),
    })
}

pub fn q4_security_labels_to_phase_b_labels(
    labels: &[Q4SecurityLabel],
) -> Result<ExtractedPhaseBLabels, UtmlError> {
    let mut extracted = ExtractedPhaseBLabels {
        source: LabelSource::PythonSecurityAnalyzer,
        failure_modes: Vec::new(),
        failed_tests: Vec::new(),
        edge_cases: Vec::new(),
        latent_bugs: Vec::new(),
        security_concerns: Vec::new(),
        reasoning_class: ReasoningClass::Correct,
        touched_files: Vec::new(),
    };
    for label in labels {
        validate_q4_security_label(label)?;
        extracted.security_concerns.push(SecurityLabel {
            class: map_q4_security_class(label.class),
            evidence_line: format!(
                "q4_security direction={} detector={} rule={} file={} line={} finding_id={}",
                if label.introduced_by_patch {
                    "introduced"
                } else {
                    "fixed"
                },
                label.detector.as_str(),
                label.rule_id,
                label.file,
                label.line_range.start_line,
                label.finding_id
            ),
        });
        let path = PathBuf::from(&label.file);
        if !extracted.touched_files.contains(&path) {
            extracted.touched_files.push(path);
        }
    }
    validate_extracted_labels(&extracted)?;
    Ok(extracted)
}

pub fn q4_perf_labels_to_prediction_surfaces(
    labels: &[Q4PerfLabel],
) -> Result<PhaseBPredictionSurfaces, UtmlError> {
    let mut surfaces = PhaseBPredictionSurfaces::default();
    for label in labels {
        validate_q4_perf_label(label)?;
        if label.category == Q4PerfCategory::WallclockBudgetExceeded {
            continue;
        }
        if label.regression {
            surfaces
                .predicted_perf_regressions
                .push(PredictedPerfRegression {
                    axis: map_q4_perf_category(label.category),
                    chunk: ChunkId(label.chunk_id.clone()),
                    predicted_delta_pct: label.delta_pct.ok_or_else(|| {
                        UtmlError::new(
                            UtmlErrorCode::InvalidSignal,
                            "Q4 perf regression missing real delta",
                        )
                    })? as f32,
                    baseline_value: label.baseline_ns,
                    confidence: if label.regression { 0.85 } else { 0.7 },
                    explanation: format!(
                    "q4_perf metric={} category={} regression={} baseline_ns={:?} after_ns={:?}",
                    label.metric,
                    label.category.as_str(),
                    label.regression,
                    label.baseline_ns,
                    label.after_ns
                ),
                });
        }
    }
    for perf in &surfaces.predicted_perf_regressions {
        perf.validate().map_err(|err| {
            UtmlError::new(
                UtmlErrorCode::InvalidSignal,
                format!("Q4 perf prediction surface invalid: {err}"),
            )
        })?;
    }
    Ok(surfaces)
}

pub fn q4_accuracy_labels_to_prediction_surfaces(
    labels: &[Q4AccuracyLabel],
) -> Result<PhaseBPredictionSurfaces, UtmlError> {
    let mut surfaces = PhaseBPredictionSurfaces::default();
    for label in labels {
        validate_q4_accuracy_label(label)?;
        if label.regression {
            surfaces
                .predicted_accuracy_degradations
                .push(PredictedAccuracyDegradation {
                    metric: map_q4_accuracy_metric(label.metric_kind, &label.metric_name),
                    chunk: ChunkId(label.chunk_id.clone()),
                    predicted_delta: label.delta_pct as f32,
                    baseline_value: Some(label.baseline_value),
                    confidence: 0.85,
                    explanation: format!(
                        "q4_accuracy metric={} kind={:?} regression={} baseline={} after={} source_test={}",
                        label.metric_name,
                        label.metric_kind,
                        label.regression,
                        label.baseline_value,
                        label.after_value,
                        label.source_test
                    ),
                });
        }
    }
    for accuracy in &surfaces.predicted_accuracy_degradations {
        accuracy.validate().map_err(|err| {
            UtmlError::new(
                UtmlErrorCode::InvalidSignal,
                format!("Q4 accuracy prediction surface invalid: {err}"),
            )
        })?;
    }
    Ok(surfaces)
}

pub fn q4_cost_labels_to_prediction_surfaces(
    labels: &[Q4CostLabel],
) -> Result<PhaseBPredictionSurfaces, UtmlError> {
    let mut surfaces = PhaseBPredictionSurfaces::default();
    for label in labels {
        validate_q4_cost_label(label)?;
        if label.regression {
            surfaces
                .predicted_cost_regressions
                .push(PredictedCostRegression {
                    axis: map_q4_cost_kind(label.kind),
                    chunk: ChunkId(label.chunk_id.clone()),
                    predicted_delta: label.delta,
                    baseline_value: Some(label.baseline),
                    explanation: format!(
                        "q4_cost kind={} regression={} baseline={} after={} delta={}",
                        label.kind.as_str(),
                        label.regression,
                        label.baseline,
                        label.after,
                        label.delta
                    ),
                });
        }
    }
    for cost in &surfaces.predicted_cost_regressions {
        cost.validate().map_err(|err| {
            UtmlError::new(
                UtmlErrorCode::InvalidSignal,
                format!("Q4 cost prediction surface invalid: {err}"),
            )
        })?;
    }
    Ok(surfaces)
}

pub fn q4_reasoning_labels_to_prediction_surfaces(
    labels: &[Q4ReasoningLabel],
) -> Result<PhaseBPredictionSurfaces, UtmlError> {
    let mut surfaces = PhaseBPredictionSurfaces::default();
    let Some(label) = labels
        .iter()
        .max_by(|left, right| left.confidence.total_cmp(&right.confidence))
    else {
        surfaces.predicted_reasoning_class = ReasoningClass::Unknown;
        return Ok(surfaces);
    };
    validate_q4_reasoning_label(label)?;
    surfaces.predicted_reasoning_class = map_q4_reasoning_class(label.class);
    Ok(surfaces)
}

pub fn apply_phase_b_labels_with_transfer_gate(
    db: &DB,
    labels: &ExtractedPhaseBLabels,
    context: &LabelTransferApplicationContext,
) -> Result<AppliedPhaseBLabels, UtmlError> {
    validate_extracted_labels(labels)?;
    validate_label_transfer_context(context)?;
    let mut accepted_failure_modes = Vec::new();
    let mut rejected_failure_modes = Vec::new();
    let mut transfer_decisions = Vec::new();
    for (idx, label) in labels.failure_modes.iter().enumerate() {
        let label_source = format!("{}:failure_mode:{idx}", context.label_source_id);
        let decision = apply_and_persist_label_transfer_decision(
            db,
            LabelTransferApplicationInput {
                prediction_id: context.prediction_id,
                label_source,
                source_chunk_id: context.source_chunk_id.clone(),
                target_chunk_id: context.target_chunk_id.clone(),
                source_language: context.source_language,
                target_language: context.target_language,
                source_root_cause: label.root_cause_class,
                target_root_cause: context.target_root_cause,
                similarity: context.similarity,
                source_cross_language_correlation: context.source_cross_language_correlation,
                created_at_unix_ms: context.created_at_unix_ms,
            },
        )
        .map_err(map_label_transfer_error)?;
        if decision.applied_weight > 0.0 {
            accepted_failure_modes.push(AppliedFailureModeLabel {
                label: label.clone(),
                effective_weight: decision.applied_weight,
                decision: decision.clone(),
            });
        } else {
            rejected_failure_modes.push(RejectedFailureModeLabel {
                label: label.clone(),
                decision: decision.clone(),
            });
        }
        transfer_decisions.push(decision);
    }
    Ok(AppliedPhaseBLabels {
        source: labels.source.clone(),
        accepted_failure_modes,
        rejected_failure_modes,
        transfer_decisions,
    })
}

fn parse_failed_test_id(line: &str, source: &LabelSource) -> Option<TestId> {
    match source {
        LabelSource::Pytest => line
            .strip_prefix("FAILED ")
            .and_then(|rest| rest.split_whitespace().next())
            .map(|raw| TestId(raw.to_string())),
        LabelSource::CargoTest => line
            .strip_prefix("test ")
            .and_then(|rest| rest.strip_suffix(" ... FAILED"))
            .map(|raw| TestId(raw.to_string())),
        LabelSource::PythonUnittest => {
            if line.starts_with("FAIL: ") || line.starts_with("ERROR: ") {
                line.split_whitespace()
                    .nth(1)
                    .map(|raw| TestId(raw.to_string()))
            } else {
                None
            }
        }
        LabelSource::CargoClippy
        | LabelSource::PythonAccuracyAnalyzer
        | LabelSource::PythonPerfAnalyzer
        | LabelSource::PythonSecurityAnalyzer
        | LabelSource::AgentResponse => None,
    }
}

fn classify_failure_line(line: &str) -> Option<FailureModeClass> {
    let lower = line.to_ascii_lowercase();
    if lower.contains("assertionerror") || lower.contains("assertion failed") {
        Some(FailureModeClass::AssertionMismatch)
    } else if lower.contains("typeerror") || lower.contains("mismatched types") {
        Some(FailureModeClass::TypeError)
    } else if lower.contains("nameerror") || lower.contains("cannot find") {
        Some(FailureModeClass::NameError)
    } else if lower.contains("syntaxerror") || lower.contains("expected one of") {
        Some(FailureModeClass::SyntaxError)
    } else if lower.contains("timeout") || lower.contains("timed out") {
        Some(FailureModeClass::Timeout)
    } else if lower.contains("panic") || lower.contains("thread '") {
        Some(FailureModeClass::Crash)
    } else if lower.contains("no tests collected") {
        Some(FailureModeClass::NoTestsCollected)
    } else if lower.contains("compile error") || lower.contains("could not compile") {
        Some(FailureModeClass::CompileError)
    } else {
        None
    }
}

fn root_cause_for_failure(failure: FailureModeClass) -> RootCauseClass {
    match failure {
        FailureModeClass::AssertionMismatch
        | FailureModeClass::WrongAlgorithm
        | FailureModeClass::OffByOne => RootCauseClass::LogicError,
        FailureModeClass::RaceCondition
        | FailureModeClass::DeadlockPotential
        | FailureModeClass::Flaky => RootCauseClass::ConcurrencyError,
        FailureModeClass::SignatureMismatch
        | FailureModeClass::TypeError
        | FailureModeClass::ImportMissing
        | FailureModeClass::NameError
        | FailureModeClass::CompileError => RootCauseClass::InterfaceError,
        FailureModeClass::ConfigDrift
        | FailureModeClass::DependencyVersionConflict
        | FailureModeClass::CompilerVersionGap
        | FailureModeClass::PlatformAssumption => RootCauseClass::EnvironmentError,
        FailureModeClass::NoTestsCollected | FailureModeClass::WrongTestUpdated => {
            RootCauseClass::TestQualityError
        }
        FailureModeClass::ResourceLeak
        | FailureModeClass::StackOverflow
        | FailureModeClass::Timeout => RootCauseClass::ResourceError,
        _ => RootCauseClass::Other,
    }
}

fn classify_edge_case_line(line: &str) -> Option<EdgeCaseClass> {
    let lower = line.to_ascii_lowercase();
    if lower.contains("empty") {
        Some(EdgeCaseClass::EmptyInput)
    } else if lower.contains("unicode") || lower.contains("utf-8") {
        Some(EdgeCaseClass::UnicodeEdge)
    } else if lower.contains("permission denied") {
        Some(EdgeCaseClass::PermissionDenied)
    } else if lower.contains("timezone") {
        Some(EdgeCaseClass::TimezoneTransition)
    } else if lower.contains("duplicate") {
        Some(EdgeCaseClass::DuplicateKeys)
    } else {
        None
    }
}

fn classify_latent_bug_line(line: &str) -> Option<LatentBugClass> {
    let lower = line.to_ascii_lowercase();
    if lower.contains("unused variable") || lower.contains("unused import") {
        Some(LatentBugClass::UnreachableBranch)
    } else if lower.contains("bare except") || lower.contains("except:") {
        Some(LatentBugClass::BareException)
    } else if lower.contains("forgotten await") || lower.contains("not awaited") {
        Some(LatentBugClass::ForgottenAwait)
    } else {
        None
    }
}

fn classify_security_line(line: &str) -> Option<SecurityConcernClass> {
    let lower = line.to_ascii_lowercase();
    if lower.contains("hardcoded password") || lower.contains("api_key") || lower.contains("secret")
    {
        Some(SecurityConcernClass::HardcodedSecret)
    } else if lower.contains("md5") || lower.contains("sha1") {
        Some(SecurityConcernClass::InsecureCryptoAlgo)
    } else if lower.contains("sql injection") {
        Some(SecurityConcernClass::SqlInjection)
    } else {
        None
    }
}

fn map_q4_security_class(class: Q4SecurityClass) -> SecurityConcernClass {
    match class {
        Q4SecurityClass::SqlInjection => SecurityConcernClass::SqlInjection,
        Q4SecurityClass::CommandInjection => SecurityConcernClass::CommandInjection,
        Q4SecurityClass::PathTraversal => SecurityConcernClass::PathTraversal,
        Q4SecurityClass::Xss => SecurityConcernClass::Xss,
        Q4SecurityClass::Csrf => SecurityConcernClass::Csrf,
        Q4SecurityClass::Ssrf => SecurityConcernClass::Ssrf,
        Q4SecurityClass::Deserialization => SecurityConcernClass::Deserialization,
        Q4SecurityClass::HardcodedSecret => SecurityConcernClass::HardcodedSecret,
        Q4SecurityClass::LoggingSecret => SecurityConcernClass::LoggingSecret,
        Q4SecurityClass::InsecureCryptoAlgo => SecurityConcernClass::InsecureCryptoAlgo,
        Q4SecurityClass::InsufficientCryptoKeyLength => {
            SecurityConcernClass::InsufficientCryptoKeyLength
        }
        Q4SecurityClass::MissingAuth => SecurityConcernClass::MissingAuth,
        Q4SecurityClass::BrokenAccessControl => SecurityConcernClass::BrokenAccessControl,
        Q4SecurityClass::OpenRedirect => SecurityConcernClass::OpenRedirect,
        Q4SecurityClass::MissingTlsVerify => SecurityConcernClass::MissingTlsVerify,
        Q4SecurityClass::Other => SecurityConcernClass::Other,
    }
}

fn map_q4_perf_category(category: Q4PerfCategory) -> PerfAxis {
    match category {
        Q4PerfCategory::CpuMs | Q4PerfCategory::WallclockBudgetExceeded => PerfAxis::CpuMs,
        Q4PerfCategory::WallclockMs | Q4PerfCategory::Improvement => PerfAxis::WallclockMs,
        Q4PerfCategory::AllocCount => PerfAxis::HeapAllocs,
        Q4PerfCategory::RssKb => PerfAxis::RssKb,
    }
}

fn map_q4_accuracy_metric(kind: Q4AccuracyMetricKind, metric_name: &str) -> AccuracyMetric {
    match kind {
        Q4AccuracyMetricKind::Accuracy => AccuracyMetric::Accuracy,
        Q4AccuracyMetricKind::F1 => AccuracyMetric::F1,
        Q4AccuracyMetricKind::Precision => AccuracyMetric::Precision,
        Q4AccuracyMetricKind::Recall => AccuracyMetric::Recall,
        Q4AccuracyMetricKind::Auc => AccuracyMetric::Auc,
        Q4AccuracyMetricKind::MeanAbsoluteError => AccuracyMetric::MeanAbsoluteError,
        Q4AccuracyMetricKind::MeanSquaredError => AccuracyMetric::MeanSquaredError,
        Q4AccuracyMetricKind::R2 => AccuracyMetric::R2,
        Q4AccuracyMetricKind::Rouge => AccuracyMetric::DownstreamTaskScore(metric_name.to_string()),
        Q4AccuracyMetricKind::Loss
        | Q4AccuracyMetricKind::LogLoss
        | Q4AccuracyMetricKind::CrossEntropy => AccuracyMetric::Other(metric_name.to_string()),
        Q4AccuracyMetricKind::Perplexity => AccuracyMetric::PerplexitySnapshot,
        Q4AccuracyMetricKind::CalibrationError => AccuracyMetric::CalibrationError,
        Q4AccuracyMetricKind::BrierScore => AccuracyMetric::BrierScore,
        Q4AccuracyMetricKind::Other => AccuracyMetric::Other(metric_name.to_string()),
    }
}

fn map_q4_cost_kind(kind: Q4CostKind) -> CostAxis {
    match kind {
        Q4CostKind::CiMinutes => CostAxis::CiMinutes,
        Q4CostKind::DependencyCount => CostAxis::DependencyCount,
        Q4CostKind::WheelBytes => CostAxis::StorageBytes,
    }
}

fn map_q4_reasoning_class(class: Q4ReasoningClass) -> ReasoningClass {
    match class {
        Q4ReasoningClass::Unknown => ReasoningClass::Unknown,
        Q4ReasoningClass::None => ReasoningClass::None,
        Q4ReasoningClass::CodeOnly => ReasoningClass::CodeOnly,
        Q4ReasoningClass::Unsupported => ReasoningClass::Unsupported,
        Q4ReasoningClass::Hedging => ReasoningClass::Hedging,
        Q4ReasoningClass::Overclaiming => ReasoningClass::Overclaiming,
        Q4ReasoningClass::Calibrated => ReasoningClass::Calibrated,
        Q4ReasoningClass::Apologetic => ReasoningClass::Apologetic,
        Q4ReasoningClass::ConfidentCorrect => ReasoningClass::ConfidentCorrect,
        Q4ReasoningClass::ConfidentWrong => ReasoningClass::ConfidentWrong,
    }
}

fn validate_q4_security_label(label: &Q4SecurityLabel) -> Result<(), UtmlError> {
    if label.finding_id.trim().is_empty()
        || label.rule_id.trim().is_empty()
        || label.file.trim().is_empty()
    {
        return Err(UtmlError::new(
            UtmlErrorCode::InvalidSignal,
            "Q4 security label has empty finding_id, rule_id, or file",
        ));
    }
    if label.introduced_by_patch == label.fixed_by_patch {
        return Err(UtmlError::new(
            UtmlErrorCode::InvalidSignal,
            "Q4 security label must have exactly one delta direction",
        ));
    }
    Ok(())
}

fn validate_q4_perf_label(label: &Q4PerfLabel) -> Result<(), UtmlError> {
    if label.corpus_row_id.trim().is_empty()
        || label.chunk_id.trim().is_empty()
        || label.metric.trim().is_empty()
    {
        return Err(UtmlError::new(
            UtmlErrorCode::InvalidSignal,
            "Q4 perf label missing corpus_row_id/chunk_id/metric",
        ));
    }
    match label.category {
        Q4PerfCategory::WallclockBudgetExceeded => {
            if label.baseline_ns.is_some() || label.after_ns.is_some() || label.delta_pct.is_some()
            {
                return Err(UtmlError::new(
                    UtmlErrorCode::InvalidSignal,
                    "Q4 perf budget-exceeded label must not carry fake deltas",
                ));
            }
        }
        _ => {
            if label.baseline_ns.is_none() || label.after_ns.is_none() || label.delta_pct.is_none()
            {
                return Err(UtmlError::new(
                    UtmlErrorCode::InvalidSignal,
                    "Q4 perf delta label missing baseline/after/delta",
                ));
            }
        }
    }
    Ok(())
}

fn validate_q4_accuracy_label(label: &Q4AccuracyLabel) -> Result<(), UtmlError> {
    if label.corpus_row_id.trim().is_empty()
        || label.chunk_id.trim().is_empty()
        || label.metric_name.trim().is_empty()
        || label.source_test.trim().is_empty()
    {
        return Err(UtmlError::new(
            UtmlErrorCode::InvalidSignal,
            "Q4 accuracy label missing corpus_row_id/chunk_id/metric_name/source_test",
        ));
    }
    if !label.baseline_value.is_finite()
        || !label.after_value.is_finite()
        || !label.delta_pct.is_finite()
    {
        return Err(UtmlError::new(
            UtmlErrorCode::InvalidSignal,
            "Q4 accuracy label carries a non-finite baseline/after/delta",
        ));
    }
    Ok(())
}

fn validate_q4_cost_label(label: &Q4CostLabel) -> Result<(), UtmlError> {
    if label.corpus_row_id.trim().is_empty()
        || label.chunk_id.trim().is_empty()
        || label.logical_path.trim().is_empty()
        || label.cost_selector.trim().is_empty()
    {
        return Err(UtmlError::new(
            UtmlErrorCode::InvalidSignal,
            "Q4 cost label missing corpus_row_id/chunk_id/logical_path/cost_selector",
        ));
    }
    if !label.baseline.is_finite() || !label.after.is_finite() || !label.delta.is_finite() {
        return Err(UtmlError::new(
            UtmlErrorCode::InvalidSignal,
            "Q4 cost label carries a non-finite baseline/after/delta",
        ));
    }
    if label.baseline < 0.0 || label.after < 0.0 {
        return Err(UtmlError::new(
            UtmlErrorCode::InvalidSignal,
            "Q4 cost label carries a negative baseline/after value",
        ));
    }
    let expected_delta = label.after - label.baseline;
    if (label.delta - expected_delta).abs() > 1e-9_f64.max(expected_delta.abs() * 1e-9) {
        return Err(UtmlError::new(
            UtmlErrorCode::InvalidSignal,
            "Q4 cost label delta disagrees with after-baseline",
        ));
    }
    let expected_kind = if label.delta > 0.0 {
        Q4CostLabelKind::Regression
    } else if label.delta < 0.0 {
        Q4CostLabelKind::Improvement
    } else {
        Q4CostLabelKind::Stable
    };
    if label.label_kind != expected_kind {
        return Err(UtmlError::new(
            UtmlErrorCode::InvalidSignal,
            "Q4 cost label_kind disagrees with delta direction",
        ));
    }
    if label.regression != (label.label_kind == Q4CostLabelKind::Regression) {
        return Err(UtmlError::new(
            UtmlErrorCode::InvalidSignal,
            "Q4 cost regression boolean disagrees with label_kind",
        ));
    }
    if label.regression && label.delta <= 0.0 {
        return Err(UtmlError::new(
            UtmlErrorCode::InvalidSignal,
            "Q4 cost regression label must carry a positive delta",
        ));
    }
    Ok(())
}

fn validate_q4_reasoning_label(label: &Q4ReasoningLabel) -> Result<(), UtmlError> {
    if label.corpus_row_id.trim().is_empty()
        || label.chunk_id.trim().is_empty()
        || label.session_id.trim().is_empty()
        || label.reason.trim().is_empty()
    {
        return Err(UtmlError::new(
            UtmlErrorCode::InvalidSignal,
            "Q4 reasoning label missing corpus_row_id/chunk_id/session_id/reason",
        ));
    }
    if !label.confidence.is_finite() || !(0.0..=1.0).contains(&label.confidence) {
        return Err(UtmlError::new(
            UtmlErrorCode::InvalidSignal,
            "Q4 reasoning label confidence must be finite in [0,1]",
        ));
    }
    Ok(())
}

fn parse_file_path(line: &str) -> Option<PathBuf> {
    line.split(|ch: char| ch.is_whitespace() || matches!(ch, ':' | ',' | ';' | '(' | ')'))
        .find(|token| {
            !token.contains("..")
                && (token.ends_with(".rs")
                    || token.ends_with(".py")
                    || token.ends_with(".ts")
                    || token.ends_with(".js")
                    || token.ends_with(".go"))
        })
        .map(PathBuf::from)
}

pub fn require_non_empty_labels(labels: &ExtractedPhaseBLabels) -> Result<(), UtmlError> {
    if labels.failure_modes.is_empty()
        && labels.failed_tests.is_empty()
        && labels.edge_cases.is_empty()
        && labels.latent_bugs.is_empty()
        && labels.security_concerns.is_empty()
    {
        return Err(UtmlError::new(
            UtmlErrorCode::InvalidSignal,
            "label extraction found no supported labels in the provided evidence",
        ));
    }
    Ok(())
}

pub fn validate_extracted_labels(labels: &ExtractedPhaseBLabels) -> Result<(), UtmlError> {
    for label in &labels.failed_tests {
        if label.test_id.0.trim().is_empty() {
            return Err(UtmlError::new(
                UtmlErrorCode::InvalidSignal,
                "failed test label has empty test_id",
            ));
        }
        if label.evidence_line.trim().is_empty() {
            return Err(UtmlError::new(
                UtmlErrorCode::InvalidSignal,
                "failed test label has empty evidence_line",
            ));
        }
    }
    for label in &labels.failure_modes {
        if label.evidence_line.trim().is_empty() {
            return Err(UtmlError::new(
                UtmlErrorCode::InvalidSignal,
                "failure mode label has empty evidence_line",
            ));
        }
    }
    for label in &labels.edge_cases {
        if label.evidence_line.trim().is_empty() {
            return Err(UtmlError::new(
                UtmlErrorCode::InvalidSignal,
                "edge-case label has empty evidence_line",
            ));
        }
    }
    for label in &labels.latent_bugs {
        if label.evidence_line.trim().is_empty() {
            return Err(UtmlError::new(
                UtmlErrorCode::InvalidSignal,
                "latent-bug label has empty evidence_line",
            ));
        }
    }
    for label in &labels.security_concerns {
        if label.evidence_line.trim().is_empty() {
            return Err(UtmlError::new(
                UtmlErrorCode::InvalidSignal,
                "security label has empty evidence_line",
            ));
        }
    }
    Ok(())
}

fn validate_analyzer_config(config: &PhaseBLabelAnalyzerConfig) -> Result<(), UtmlError> {
    if config.program.as_os_str().is_empty() {
        return Err(UtmlError::new(
            UtmlErrorCode::MissingSourceOfTruth,
            "phase-b label analyzer program is empty",
        ));
    }
    if config.cwd.as_os_str().is_empty() {
        return Err(UtmlError::new(
            UtmlErrorCode::MissingSourceOfTruth,
            "phase-b label analyzer cwd is empty",
        ));
    }
    if !(1..=1_048_576).contains(&config.max_output_bytes) {
        return Err(UtmlError::new(
            UtmlErrorCode::InvalidSignal,
            format!(
                "phase-b label analyzer max_output_bytes must be in [1, 1048576], got {}",
                config.max_output_bytes
            ),
        ));
    }
    if config.args.len() > 128 {
        return Err(UtmlError::new(
            UtmlErrorCode::InvalidSignal,
            format!(
                "phase-b label analyzer has {} args; max is 128",
                config.args.len()
            ),
        ));
    }
    if !source_supports_language(&config.source, config.language) {
        return Err(UtmlError::new(
            UtmlErrorCode::InvalidSignal,
            format!(
                "analyzer source {:?} does not support input language {:?}",
                config.source, config.language
            ),
        ));
    }
    Ok(())
}

fn source_supports_language(source: &LabelSource, language: LabelInputLanguage) -> bool {
    match source {
        LabelSource::Pytest | LabelSource::PythonUnittest => language == LabelInputLanguage::Python,
        LabelSource::PythonPerfAnalyzer | LabelSource::PythonSecurityAnalyzer => {
            language == LabelInputLanguage::Python
        }
        LabelSource::PythonAccuracyAnalyzer => language == LabelInputLanguage::Python,
        LabelSource::CargoTest | LabelSource::CargoClippy => language == LabelInputLanguage::Rust,
        LabelSource::AgentResponse => false,
    }
}

fn validate_label_transfer_context(
    context: &LabelTransferApplicationContext,
) -> Result<(), UtmlError> {
    if context.prediction_id.0 == [0_u8; 16] {
        return Err(UtmlError::new(
            UtmlErrorCode::InvalidSignal,
            "label transfer prediction_id must be non-zero",
        ));
    }
    if context.label_source_id.trim().is_empty() {
        return Err(UtmlError::new(
            UtmlErrorCode::MissingSourceOfTruth,
            "label transfer source id is empty",
        ));
    }
    context
        .source_chunk_id
        .validate("label_transfer.source_chunk_id")
        .map_err(|err| UtmlError::new(UtmlErrorCode::InvalidSignal, err.to_string()))?;
    context
        .target_chunk_id
        .validate("label_transfer.target_chunk_id")
        .map_err(|err| UtmlError::new(UtmlErrorCode::InvalidSignal, err.to_string()))?;
    if context.created_at_unix_ms < 0 {
        return Err(UtmlError::new(
            UtmlErrorCode::InvalidSignal,
            "label transfer created_at_unix_ms must be non-negative",
        ));
    }
    Ok(())
}

fn map_label_transfer_error(err: context_graph_mejepa::LabelTransferAuditError) -> UtmlError {
    let code = match &err {
        context_graph_mejepa::LabelTransferAuditError::MissingColumnFamily(_) => {
            UtmlErrorCode::MissingSourceOfTruth
        }
        context_graph_mejepa::LabelTransferAuditError::ReadbackMismatch(_) => {
            UtmlErrorCode::ReadbackMismatch
        }
        context_graph_mejepa::LabelTransferAuditError::RocksDb(_) => UtmlErrorCode::Io,
        context_graph_mejepa::LabelTransferAuditError::Bincode(_) => UtmlErrorCode::InvalidSignal,
        context_graph_mejepa::LabelTransferAuditError::InvalidInput { .. }
        | context_graph_mejepa::LabelTransferAuditError::Policy(_) => UtmlErrorCode::InvalidSignal,
    };
    UtmlError::new(code, err.to_string())
}

fn bounded_output(field: &str, bytes: &[u8], max_bytes: usize) -> Result<String, UtmlError> {
    if bytes.len() > max_bytes {
        return Err(UtmlError::new(
            UtmlErrorCode::InvalidSignal,
            format!(
                "analyzer {field} is {} bytes; max is {max_bytes}",
                bytes.len()
            ),
        ));
    }
    String::from_utf8(bytes.to_vec()).map_err(|err| {
        UtmlError::new(
            UtmlErrorCode::InvalidSignal,
            format!("analyzer {field} is not UTF-8: {err}"),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use context_graph_mejepa_instruments::{
        Q4AccuracyLabelKind, Q4SecurityDetector, Q4SecurityLineRange, Q4SecuritySeverity,
    };

    #[test]
    fn pytest_extracts_failure_and_empty_edge() {
        let log = "FAILED tests/test_parser.py::test_empty - AssertionError: empty input rejected";
        let labels = extract_pytest_labels(log, "", 1).unwrap();
        assert_eq!(
            labels.failed_tests[0].test_id.0,
            "tests/test_parser.py::test_empty"
        );
        assert_eq!(
            labels.failure_modes[0].failure_class,
            FailureModeClass::AssertionMismatch
        );
        assert_eq!(labels.edge_cases[0].edge_class, EdgeCaseClass::EmptyInput);
        require_non_empty_labels(&labels).unwrap();
    }

    #[test]
    fn empty_output_fails_closed() {
        let err = extract_pytest_labels("", "", 1).unwrap_err();
        assert_eq!(err.code(), "UTML_EMPTY_INPUT");
    }

    #[test]
    fn q4_security_labels_feed_phase_b_security_adapter() {
        let labels = q4_security_labels_to_phase_b_labels(&[Q4SecurityLabel {
            corpus_row_id: "row-q4".to_string(),
            chunk_id: "chunk-q4".to_string(),
            finding_id: "bandit:abc".to_string(),
            rule_id: "B602".to_string(),
            severity: Q4SecuritySeverity::High,
            class: Q4SecurityClass::CommandInjection,
            file: "app/security.py".to_string(),
            line_range: Q4SecurityLineRange {
                start_line: 7,
                end_line: 7,
                start_column: 1,
                end_column: 32,
            },
            detector: Q4SecurityDetector::Bandit,
            message: "shell=True".to_string(),
            introduced_by_patch: true,
            fixed_by_patch: false,
        }])
        .unwrap();
        assert_eq!(labels.source, LabelSource::PythonSecurityAnalyzer);
        assert_eq!(
            labels.security_concerns[0].class,
            SecurityConcernClass::CommandInjection
        );
        assert_eq!(labels.touched_files[0], PathBuf::from("app/security.py"));
        require_non_empty_labels(&labels).unwrap();
    }

    #[test]
    fn q4_perf_labels_feed_prediction_surface_adapter() {
        let surfaces = q4_perf_labels_to_prediction_surfaces(&[Q4PerfLabel {
            corpus_row_id: "row-q4-perf".to_string(),
            chunk_id: "chunk-q4-perf".to_string(),
            metric: "test_linear".to_string(),
            category: Q4PerfCategory::WallclockMs,
            baseline_ns: Some(1_000_000.0),
            after_ns: Some(1_500_000.0),
            delta_pct: Some(50.0),
            regression: true,
        }])
        .unwrap();
        assert_eq!(surfaces.predicted_perf_regressions.len(), 1);
        assert_eq!(
            surfaces.predicted_perf_regressions[0].axis,
            PerfAxis::WallclockMs
        );
        assert_eq!(
            surfaces.predicted_perf_regressions[0].chunk,
            ChunkId("chunk-q4-perf".to_string())
        );
    }

    #[test]
    fn q4_perf_budget_signal_does_not_fabricate_delta_surface() {
        let surfaces = q4_perf_labels_to_prediction_surfaces(&[Q4PerfLabel {
            corpus_row_id: "row-q4-perf-budget".to_string(),
            chunk_id: "chunk-q4-perf".to_string(),
            metric: "cprofile_walltime_budget".to_string(),
            category: Q4PerfCategory::WallclockBudgetExceeded,
            baseline_ns: None,
            after_ns: None,
            delta_pct: None,
            regression: true,
        }])
        .unwrap();
        assert!(surfaces.predicted_perf_regressions.is_empty());
    }

    #[test]
    fn q4_accuracy_labels_feed_prediction_surface_adapter() {
        let surfaces = q4_accuracy_labels_to_prediction_surfaces(&[Q4AccuracyLabel {
            corpus_row_id: "row-q4-accuracy".to_string(),
            chunk_id: "chunk-q4-accuracy".to_string(),
            metric_name: "accuracy".to_string(),
            metric_kind: Q4AccuracyMetricKind::Accuracy,
            baseline_value: 0.91,
            after_value: 0.83,
            delta_pct: -8.791_209,
            regression: true,
            kind: Q4AccuracyLabelKind::Regression,
            source_test: "tests/test_model.py::test_quality".to_string(),
        }])
        .unwrap();
        assert_eq!(surfaces.predicted_accuracy_degradations.len(), 1);
        assert_eq!(
            surfaces.predicted_accuracy_degradations[0].metric,
            AccuracyMetric::Accuracy
        );
        assert_eq!(
            surfaces.predicted_accuracy_degradations[0].chunk,
            ChunkId("chunk-q4-accuracy".to_string())
        );
    }

    #[test]
    fn q4_cost_labels_feed_prediction_surface_adapter() {
        let surfaces = q4_cost_labels_to_prediction_surfaces(&[Q4CostLabel {
            corpus_row_id: "row-q4-cost".to_string(),
            chunk_id: "chunk-q4-cost".to_string(),
            logical_path: "requirements.txt".to_string(),
            cost_selector: "pytest-and-build-wheel".to_string(),
            kind: Q4CostKind::DependencyCount,
            baseline: 12.0,
            after: 14.0,
            delta: 2.0,
            regression: true,
            label_kind: context_graph_mejepa_instruments::Q4CostLabelKind::Regression,
        }])
        .unwrap();
        assert_eq!(surfaces.predicted_cost_regressions.len(), 1);
        assert_eq!(
            surfaces.predicted_cost_regressions[0].axis,
            CostAxis::DependencyCount
        );
        assert_eq!(
            surfaces.predicted_cost_regressions[0].chunk,
            ChunkId("chunk-q4-cost".to_string())
        );
    }

    #[test]
    fn q4_cost_improvements_do_not_fabricate_regression_surface() {
        let surfaces = q4_cost_labels_to_prediction_surfaces(&[Q4CostLabel {
            corpus_row_id: "row-q4-cost-fix".to_string(),
            chunk_id: "chunk-q4-cost".to_string(),
            logical_path: "requirements.txt".to_string(),
            cost_selector: "pytest-and-build-wheel".to_string(),
            kind: Q4CostKind::DependencyCount,
            baseline: 14.0,
            after: 12.0,
            delta: -2.0,
            regression: false,
            label_kind: context_graph_mejepa_instruments::Q4CostLabelKind::Improvement,
        }])
        .unwrap();
        assert!(surfaces.predicted_cost_regressions.is_empty());
    }

    #[test]
    fn q4_reasoning_labels_feed_prediction_surface_adapter() {
        let surfaces = q4_reasoning_labels_to_prediction_surfaces(&[Q4ReasoningLabel {
            corpus_row_id: "row-q4-reasoning".to_string(),
            chunk_id: "chunk-q4-reasoning".to_string(),
            session_id: "session-q4-reasoning".to_string(),
            class: Q4ReasoningClass::Overclaiming,
            confidence: 0.92,
            reason: "agent claimed success while oracle reality failed".to_string(),
            oracle_outcome: context_graph_mejepa_instruments::Q4ReasoningOutcome::Fail,
            prediction_verdict:
                context_graph_mejepa_instruments::Q4ReasoningPredictionVerdict::Fail,
            features: context_graph_mejepa_instruments::Q4ReasoningFeatures {
                hedge_terms: 0,
                apology_terms: 0,
                confidence_terms: 1,
                success_claim_terms: 1,
                failure_ack_terms: 0,
                natural_language_words: 8,
                code_like_lines: 0,
                non_ascii_ratio: 0.0,
                pairwise_cosine_reasoning_diff: 0.2,
                pairwise_cosine_reasoning_oracle: 0.1,
                e17_affect_score: 0.7,
                oracle_pass: false,
            },
        }])
        .unwrap();
        assert_eq!(
            surfaces.predicted_reasoning_class,
            ReasoningClass::Overclaiming
        );
    }
}
