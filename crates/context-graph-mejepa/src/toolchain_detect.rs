//! TASK-PREDICT-LABEL-010 — Per-language toolchain availability audit.
//!
//! Each label-source extractor declares the binaries it needs at
//! startup. `detect_required_toolchains()` walks `PATH` for each one;
//! the first missing binary raises a structured
//! `MEJEPA_LABEL_TOOLCHAIN_MISSING` error so the daemon refuses to
//! start with partially-available extractors. No silent "unavailable:
//! true" labels are ever persisted.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::types::Language;

/// A single binary that an extractor needs.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ToolchainBinary {
    pub binary: String,
    pub language: Language,
    pub extractor: String,
    pub remediation: String,
}

impl ToolchainBinary {
    pub fn new(binary: &str, language: Language, extractor: &str, remediation: &str) -> Self {
        Self {
            binary: binary.to_string(),
            language,
            extractor: extractor.to_string(),
            remediation: remediation.to_string(),
        }
    }

    fn validate(&self) -> Result<(), ToolchainAuditError> {
        validate_non_empty("binary", &self.binary)?;
        validate_non_empty("extractor", &self.extractor)?;
        validate_non_empty("remediation", &self.remediation)?;
        Ok(())
    }
}

/// Resolution outcome for one binary.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedBinary {
    pub binary: String,
    pub language: Language,
    pub extractor: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ToolchainAuditReport {
    pub required_count: usize,
    pub path_entries_checked: usize,
    pub resolved: Vec<ResolvedBinary>,
    pub missing: Vec<ToolchainMissingDiagnostic>,
    pub all_available: bool,
    pub no_partial_labels_persisted: bool,
}

impl ToolchainAuditReport {
    pub fn first_missing(&self) -> Option<&ToolchainMissingDiagnostic> {
        self.missing.first()
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ToolchainMissingDiagnostic {
    pub binary: String,
    pub language: Language,
    pub extractor: String,
    pub remediation: String,
    pub path_entries_checked: usize,
    pub error_code: &'static str,
}

#[derive(Debug, thiserror::Error)]
#[error(
    "MEJEPA_LABEL_TOOLCHAIN_MISSING: extractor={extractor} language={language:?} binary={binary} \
     not found on PATH; remediation: {remediation}"
)]
pub struct ToolchainMissingError {
    pub binary: String,
    pub language: Language,
    pub extractor: String,
    pub remediation: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ToolchainAuditError {
    #[error("MEJEPA_LABEL_TOOLCHAIN_INVALID_REQUIREMENT: {field} must be non-empty and contain no control characters")]
    InvalidRequirement { field: &'static str },
    #[error(transparent)]
    Missing(#[from] ToolchainMissingError),
}

pub fn default_enabled_label_toolchains() -> Vec<ToolchainBinary> {
    vec![
        ToolchainBinary::new(
            "ruff",
            Language::Python,
            "python_static_analysis",
            "install ruff for Python static-analysis labels",
        ),
        ToolchainBinary::new(
            "mypy",
            Language::Python,
            "python_static_analysis",
            "install mypy for Python static-analysis labels",
        ),
        ToolchainBinary::new(
            "pyright",
            Language::Python,
            "python_type_graph",
            "install pyright for Python type-graph labels",
        ),
        ToolchainBinary::new(
            "bandit",
            Language::Python,
            "python_security_analysis",
            "install bandit for Python Q4 security labels",
        ),
        ToolchainBinary::new(
            "semgrep",
            Language::Python,
            "python_security_analysis",
            "install semgrep for Python Q4 security labels",
        ),
        ToolchainBinary::new(
            "python3",
            Language::Python,
            "python_runtime_compile",
            "install python3 for Python compile/runtime labels",
        ),
        ToolchainBinary::new(
            "cargo",
            Language::Rust,
            "rust_static_analysis",
            "install the Rust toolchain for Rust labels",
        ),
        ToolchainBinary::new(
            "clippy-driver",
            Language::Rust,
            "rust_static_analysis",
            "install the clippy component for Rust static-analysis labels",
        ),
        ToolchainBinary::new(
            "node",
            Language::Javascript,
            "javascript_runtime",
            "install node for JavaScript labels",
        ),
        ToolchainBinary::new(
            "eslint",
            Language::Javascript,
            "javascript_static_analysis",
            "install eslint for JavaScript static-analysis labels",
        ),
        ToolchainBinary::new(
            "node",
            Language::Typescript,
            "typescript_runtime",
            "install node for TypeScript labels",
        ),
        ToolchainBinary::new(
            "tsc",
            Language::Typescript,
            "typescript_type_graph",
            "install TypeScript tsc for TypeScript type-graph labels",
        ),
        ToolchainBinary::new(
            "go",
            Language::Go,
            "go_toolchain",
            "install go for Go label extraction",
        ),
        ToolchainBinary::new(
            "javac",
            Language::Java,
            "java_compile",
            "install a JDK for Java label extraction",
        ),
        ToolchainBinary::new(
            "cc",
            Language::C,
            "c_compile",
            "install a C compiler for C label extraction",
        ),
        ToolchainBinary::new(
            "c++",
            Language::Cpp,
            "cpp_compile",
            "install a C++ compiler for C++ label extraction",
        ),
        ToolchainBinary::new(
            "dotnet",
            Language::CSharp,
            "csharp_compile",
            "install dotnet for C# label extraction",
        ),
        ToolchainBinary::new(
            "ruby",
            Language::Ruby,
            "ruby_syntax",
            "install ruby for Ruby label extraction",
        ),
        ToolchainBinary::new(
            "php",
            Language::Php,
            "php_syntax",
            "install php for PHP label extraction",
        ),
    ]
}

pub fn audit_required_toolchains(
    required: &[ToolchainBinary],
    path_env: Option<&str>,
) -> Result<ToolchainAuditReport, ToolchainAuditError> {
    validate_requirements(required)?;
    let candidates = parse_path(path_env.unwrap_or_default());
    let mut resolved = Vec::new();
    let mut missing = Vec::new();

    for tool in required {
        match find_binary(&tool.binary, &candidates) {
            Some(path) => resolved.push(ResolvedBinary {
                binary: tool.binary.clone(),
                language: tool.language,
                extractor: tool.extractor.clone(),
                path,
            }),
            None => missing.push(ToolchainMissingDiagnostic {
                binary: tool.binary.clone(),
                language: tool.language,
                extractor: tool.extractor.clone(),
                remediation: tool.remediation.clone(),
                path_entries_checked: candidates.len(),
                error_code: "MEJEPA_LABEL_TOOLCHAIN_MISSING",
            }),
        }
    }

    Ok(ToolchainAuditReport {
        required_count: required.len(),
        path_entries_checked: candidates.len(),
        resolved,
        all_available: missing.is_empty(),
        no_partial_labels_persisted: missing.is_empty(),
        missing,
    })
}

/// Resolve every binary in `required` against `path_env` (typically
/// `std::env::var_os("PATH")`). Fail-closed on the first missing one.
pub fn detect_required_toolchains(
    required: &[ToolchainBinary],
    path_env: Option<&str>,
) -> Result<Vec<ResolvedBinary>, ToolchainMissingError> {
    match audit_required_toolchains(required, path_env) {
        Ok(report) if report.all_available => Ok(report.resolved),
        Ok(report) => {
            let missing = report
                .first_missing()
                .expect("audit report with all_available=false must contain a missing tool");
            Err(ToolchainMissingError {
                binary: missing.binary.clone(),
                language: missing.language,
                extractor: missing.extractor.clone(),
                remediation: missing.remediation.clone(),
            })
        }
        Err(ToolchainAuditError::Missing(err)) => Err(err),
        Err(ToolchainAuditError::InvalidRequirement { field }) => Err(ToolchainMissingError {
            binary: field.to_string(),
            language: Language::Python,
            extractor: "invalid_requirement".to_string(),
            remediation: "fix the enabled label-toolchain registry".to_string(),
        }),
    }
}

pub fn enforce_required_toolchains(
    required: &[ToolchainBinary],
    path_env: Option<&str>,
) -> Result<ToolchainAuditReport, ToolchainAuditError> {
    let report = audit_required_toolchains(required, path_env)?;
    if let Some(missing) = report.first_missing() {
        return Err(ToolchainMissingError {
            binary: missing.binary.clone(),
            language: missing.language,
            extractor: missing.extractor.clone(),
            remediation: missing.remediation.clone(),
        }
        .into());
    }
    Ok(report)
}

fn validate_requirements(required: &[ToolchainBinary]) -> Result<(), ToolchainAuditError> {
    for tool in required {
        tool.validate()?;
    }
    Ok(())
}

fn validate_non_empty(field: &'static str, value: &str) -> Result<(), ToolchainAuditError> {
    if value.trim().is_empty() || value.chars().any(char::is_control) {
        return Err(ToolchainAuditError::InvalidRequirement { field });
    }
    Ok(())
}

fn parse_path(path_env: &str) -> Vec<PathBuf> {
    if path_env.is_empty() {
        return Vec::new();
    }
    std::env::split_paths(path_env).collect()
}

fn find_binary(binary: &str, candidates: &[PathBuf]) -> Option<PathBuf> {
    for dir in candidates {
        let candidate = dir.join(binary);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn tool(binary: &str, language: Language, extractor: &str) -> ToolchainBinary {
        ToolchainBinary::new(binary, language, extractor, &format!("install {binary}"))
    }

    fn make_path_dir(tmp: &TempDir, file_name: &str) -> String {
        let path = tmp.path().to_path_buf();
        fs::write(path.join(file_name), b"#!/bin/sh\necho ok\n").unwrap();
        path.to_string_lossy().into_owned()
    }

    #[test]
    fn detects_present_binary() {
        let tmp = TempDir::new().unwrap();
        let path = make_path_dir(&tmp, "pyright");
        let resolved = detect_required_toolchains(
            &[tool("pyright", Language::Python, "type_graph")],
            Some(&path),
        )
        .unwrap();
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].binary, "pyright");
    }

    #[test]
    fn missing_binary_fails_closed() {
        let tmp = TempDir::new().unwrap();
        let path = make_path_dir(&tmp, "other-tool");
        let err = detect_required_toolchains(
            &[tool("pyright", Language::Python, "type_graph")],
            Some(&path),
        )
        .unwrap_err();
        assert_eq!(err.binary, "pyright");
        assert_eq!(err.extractor, "type_graph");
        assert!(err.to_string().contains("MEJEPA_LABEL_TOOLCHAIN_MISSING"));
    }

    #[test]
    fn empty_path_env_fails_closed() {
        let err =
            detect_required_toolchains(&[tool("pyright", Language::Python, "type_graph")], None)
                .unwrap_err();
        assert!(err.remediation.contains("install pyright"));
    }

    #[test]
    fn first_missing_binary_short_circuits() {
        let tmp = TempDir::new().unwrap();
        let path = make_path_dir(&tmp, "ruff");
        let err = detect_required_toolchains(
            &[
                tool("ruff", Language::Python, "static_analysis"),
                tool("clippy-driver", Language::Rust, "static_analysis"),
            ],
            Some(&path),
        )
        .unwrap_err();
        assert_eq!(err.binary, "clippy-driver");
    }

    #[test]
    fn audit_report_records_missing_without_partial_labels() {
        let tmp = TempDir::new().unwrap();
        let path = make_path_dir(&tmp, "ruff");
        let report = audit_required_toolchains(
            &[
                tool("ruff", Language::Python, "static_analysis"),
                tool("pyright", Language::Python, "type_graph"),
            ],
            Some(&path),
        )
        .unwrap();
        assert!(!report.all_available);
        assert!(!report.no_partial_labels_persisted);
        assert_eq!(report.path_entries_checked, 1);
        assert_eq!(report.resolved.len(), 1);
        assert_eq!(report.missing[0].path_entries_checked, 1);
        assert_eq!(
            report.missing[0].error_code,
            "MEJEPA_LABEL_TOOLCHAIN_MISSING"
        );
    }

    #[test]
    fn invalid_requirement_fails_closed() {
        let err = audit_required_toolchains(
            &[ToolchainBinary::new(
                "",
                Language::Python,
                "type_graph",
                "install pyright",
            )],
            Some("/usr/bin"),
        )
        .unwrap_err();
        assert!(matches!(
            err,
            ToolchainAuditError::InvalidRequirement { field: "binary" }
        ));
    }

    #[test]
    fn empty_required_returns_empty() {
        let resolved = detect_required_toolchains(&[], Some("/usr/bin")).unwrap();
        assert!(resolved.is_empty());
    }

    #[test]
    fn default_registry_covers_supported_label_languages() {
        let registry = default_enabled_label_toolchains();
        for language in [
            Language::Python,
            Language::Rust,
            Language::Javascript,
            Language::Typescript,
            Language::Go,
            Language::Java,
            Language::C,
            Language::Cpp,
            Language::CSharp,
            Language::Ruby,
            Language::Php,
        ] {
            assert!(
                registry.iter().any(|tool| tool.language == language),
                "missing default toolchain entry for {language:?}"
            );
        }
    }
}
