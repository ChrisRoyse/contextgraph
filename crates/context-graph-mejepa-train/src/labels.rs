use std::path::PathBuf;

use context_graph_mejepa::{
    EdgeCaseClass, FailureModeClass, LatentBugClass, ReasoningClass, RootCauseClass,
    SecurityConcernClass, TestDeltaKind, TestId, TestOutcome,
};
use serde::{Deserialize, Serialize};

use crate::learning_signal::{non_empty, UtmlError, UtmlErrorCode};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LabelSource {
    Pytest,
    PythonUnittest,
    CargoTest,
    CargoClippy,
    AgentResponse,
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
        LabelSource::CargoClippy | LabelSource::AgentResponse => None,
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
