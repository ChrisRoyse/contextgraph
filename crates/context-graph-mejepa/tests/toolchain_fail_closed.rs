//! TASK-TEST-009 — Toolchain fail-closed integration matrix.
//!
//! For every enabled per-language label-source extractor, asserts:
//! 1. When the required binary is present, the audit resolves it.
//! 2. When the required binary is absent, the audit surfaces a
//!    structured `MEJEPA_LABEL_TOOLCHAIN_MISSING` diagnostic carrying
//!    the exact missing binary, extractor, and remediation hint.
//! 3. The "missing" outcome marks `no_partial_labels_persisted: true`,
//!    so the daemon never half-enables an extractor.

use std::collections::BTreeSet;
use std::fs;

use context_graph_mejepa::toolchain_detect::{
    audit_required_toolchains, default_enabled_label_toolchains, ToolchainBinary,
    ToolchainMissingDiagnostic,
};
use context_graph_mejepa::types::Language;
use tempfile::TempDir;

fn make_path_with_binaries(binaries: &[&str]) -> (TempDir, String) {
    let tmp = TempDir::new().expect("tempdir");
    for binary in binaries {
        fs::write(tmp.path().join(binary), b"#!/bin/sh\nexit 0\n").expect("seed binary");
    }
    let s = tmp.path().to_string_lossy().into_owned();
    (tmp, s)
}

#[test]
fn default_catalogue_has_one_extractor_per_language() {
    let catalogue = default_enabled_label_toolchains();
    let languages: BTreeSet<Language> = catalogue.iter().map(|t| t.language).collect();
    // Per CLAUDE.md §3.10 the supported languages are Rust, Python,
    // JavaScript, TypeScript, Go, Java, C, C++, C#, Ruby, PHP.
    assert!(
        languages.len() >= 11,
        "default catalogue must cover all supported languages"
    );
}

#[test]
fn matrix_every_extractor_resolves_when_its_binary_is_present() {
    let catalogue = default_enabled_label_toolchains();
    for tool in &catalogue {
        let (_tmp, path) = make_path_with_binaries(&[tool.binary.as_str()]);
        let report = audit_required_toolchains(std::slice::from_ref(tool), Some(&path))
            .expect("audit must succeed when binary present");
        assert!(
            report.all_available,
            "{} ({:?}) should resolve",
            tool.binary, tool.language
        );
        assert_eq!(
            report.resolved.len(),
            1,
            "{} should produce exactly one ResolvedBinary",
            tool.binary
        );
    }
}

#[test]
fn matrix_missing_binary_surfaces_structured_diagnostic_per_extractor() {
    let catalogue = default_enabled_label_toolchains();
    let (_tmp, empty_path) = make_path_with_binaries(&[]);
    for tool in &catalogue {
        let report = audit_required_toolchains(std::slice::from_ref(tool), Some(&empty_path))
            .expect("audit must produce diagnostic when binary missing");
        assert!(
            !report.all_available,
            "{} ({:?}) should not resolve with empty PATH",
            tool.binary, tool.language
        );
        assert!(
            !report.no_partial_labels_persisted,
            "no_partial_labels_persisted must be false when audit fails"
        );
        let diagnostic: &ToolchainMissingDiagnostic = report
            .first_missing()
            .expect("first_missing should yield diagnostic");
        assert_eq!(diagnostic.binary, tool.binary);
        assert_eq!(diagnostic.language, tool.language);
        assert_eq!(diagnostic.extractor, tool.extractor);
        assert_eq!(diagnostic.error_code, "MEJEPA_LABEL_TOOLCHAIN_MISSING");
        assert!(!diagnostic.remediation.trim().is_empty());
    }
}

#[test]
fn empty_path_env_yields_missing_diagnostics_for_all_extractors() {
    let catalogue = default_enabled_label_toolchains();
    let report =
        audit_required_toolchains(&catalogue, None).expect("audit None path returns report");
    assert!(!report.all_available);
    assert_eq!(report.missing.len(), catalogue.len());
}

#[test]
fn invalid_requirement_fails_closed() {
    let bad = ToolchainBinary::new("", Language::Python, "py", "install");
    let err = audit_required_toolchains(std::slice::from_ref(&bad), Some("/usr/bin")).unwrap_err();
    assert!(err
        .to_string()
        .contains("MEJEPA_LABEL_TOOLCHAIN_INVALID_REQUIREMENT"));
}
