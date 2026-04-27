//! Unified Real Data Benchmark Validation Suite
//!
//! Comprehensive validation for the 13-embedder fingerprint system, testing
//! ARCH compliance, component behavior, and optimization opportunities.
//!
//! ## Modules
//!
//! - `config_validation` - EmbedderName enum and category tests
//! - `temporal_validation` - E2/E3/E4 temporal injector tests
//! - `ground_truth_validation` - Per-embedder ground truth tests
//! - `arch_compliance` - ARCH rule compliance checks
//! - `asymmetric_validation` - E5/E8/E10 asymmetric ratio tests
//! - `ablation_validation` - Contribution analysis tests
//! - `e1_foundation` - E1 as foundation tests
//! - `fusion_validation` - Fusion strategy tests
//!
//! ## Usage
//!
//! ```bash
//! cargo run -p context-graph-benchmark --bin validation-realdata-bench --release -- \
//!   --data-dir data/hf_benchmark_diverse \
//!   --max-chunks 10000 \
//!   --output benchmark_results/validation
//! ```

pub mod ablation_validation;
pub mod arch_compliance;
pub mod asymmetric_validation;
pub mod config_validation;
pub mod e1_foundation;
pub mod fusion_validation;
pub mod ground_truth_validation;
pub mod results_aggregator;
pub mod temporal_validation;

pub use ablation_validation::{AblationValidationResult, AblationValidator};
pub use arch_compliance::{ArchComplianceResult, ArchComplianceValidator};
pub use asymmetric_validation::{AsymmetricValidationResult, AsymmetricValidator};
pub use config_validation::{ConfigValidationResult, ConfigValidator};
pub use e1_foundation::{E1FoundationResult, E1FoundationValidator};
pub use fusion_validation::{FusionValidationResult, FusionValidator};
pub use ground_truth_validation::{GroundTruthValidationResult, GroundTruthValidator};
pub use results_aggregator::{AggregatedResults, BenchmarkResultsAggregator};
pub use temporal_validation::{TemporalValidationResult, TemporalValidator};

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Configuration for the validation benchmark.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationBenchmarkConfig {
    /// Directory containing chunks.jsonl and metadata.json.
    pub data_dir: PathBuf,
    /// Maximum chunks to load (0 = unlimited).
    pub max_chunks: usize,
    /// Phases to run (empty = all).
    pub phases: Vec<ValidationPhase>,
    /// Output directory for results.
    pub output_dir: PathBuf,
    /// Show verbose output.
    pub verbose: bool,
    /// Stop on first critical failure.
    pub fail_fast: bool,
    /// Random seed for reproducibility.
    pub seed: u64,
    /// Number of queries for metric validation.
    pub num_queries: usize,
}

impl Default for ValidationBenchmarkConfig {
    fn default() -> Self {
        Self {
            data_dir: PathBuf::from("data/hf_benchmark_diverse"),
            max_chunks: 10000,
            phases: vec![],
            output_dir: PathBuf::from("benchmark_results/validation"),
            verbose: true,
            fail_fast: false,
            seed: 42,
            num_queries: 50,
        }
    }
}

impl ValidationBenchmarkConfig {
    /// Quick validation with limited data.
    pub fn quick() -> Self {
        Self {
            max_chunks: 1000,
            num_queries: 20,
            ..Default::default()
        }
    }
}

/// Validation phases.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ValidationPhase {
    /// Config validation (EmbedderName enum).
    Config,
    /// Temporal injector validation (E2/E3/E4).
    Temporal,
    /// Ground truth validation.
    GroundTruth,
    /// ARCH compliance validation.
    Arch,
    /// Asymmetric embedder validation (E5/E8/E10).
    Asymmetric,
    /// Ablation validation.
    Ablation,
    /// E1 foundation validation.
    E1Foundation,
    /// Fusion strategy validation.
    Fusion,
}

impl ValidationPhase {
    pub fn all() -> Vec<Self> {
        vec![
            Self::Config,
            Self::Temporal,
            Self::GroundTruth,
            Self::Arch,
            Self::Asymmetric,
            Self::Ablation,
            Self::E1Foundation,
            Self::Fusion,
        ]
    }

    pub fn parse_from(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "config" => Some(Self::Config),
            "temporal" => Some(Self::Temporal),
            "groundtruth" | "ground_truth" | "gt" => Some(Self::GroundTruth),
            "arch" | "compliance" => Some(Self::Arch),
            "asymmetric" => Some(Self::Asymmetric),
            "ablation" => Some(Self::Ablation),
            "e1" | "e1foundation" | "e1_foundation" | "foundation" => Some(Self::E1Foundation),
            "fusion" => Some(Self::Fusion),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Config => "config",
            Self::Temporal => "temporal",
            Self::GroundTruth => "ground_truth",
            Self::Arch => "arch",
            Self::Asymmetric => "asymmetric",
            Self::Ablation => "ablation",
            Self::E1Foundation => "e1_foundation",
            Self::Fusion => "fusion",
        }
    }
}

/// Status of a validation check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValidationStatus {
    /// Check passed.
    Pass,
    /// Check failed (critical).
    Fail,
    /// Check skipped.
    Skip,
    /// Check produced warning (non-critical).
    Warning,
}

/// Individual validation check result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationCheck {
    /// Check name.
    pub name: String,
    /// Check description.
    pub description: String,
    /// Check status.
    pub status: ValidationStatus,
    /// Expected value (for display).
    pub expected: String,
    /// Actual value (for display).
    pub actual: String,
    /// Additional details.
    pub details: Option<String>,
    /// Priority (Critical, High, Medium, Low).
    pub priority: CheckPriority,
}

impl ValidationCheck {
    pub fn new(name: &str, description: &str) -> Self {
        Self {
            name: name.to_string(),
            description: description.to_string(),
            status: ValidationStatus::Skip,
            expected: String::new(),
            actual: String::new(),
            details: None,
            priority: CheckPriority::High,
        }
    }

    pub fn pass(mut self, actual: &str, expected: &str) -> Self {
        self.status = ValidationStatus::Pass;
        self.actual = actual.to_string();
        self.expected = expected.to_string();
        self
    }

    pub fn fail(mut self, actual: &str, expected: &str) -> Self {
        self.status = ValidationStatus::Fail;
        self.actual = actual.to_string();
        self.expected = expected.to_string();
        self
    }

    pub fn warning(mut self, actual: &str, expected: &str) -> Self {
        self.status = ValidationStatus::Warning;
        self.actual = actual.to_string();
        self.expected = expected.to_string();
        self
    }

    pub fn skip(mut self, actual: &str, expected: &str) -> Self {
        self.status = ValidationStatus::Skip;
        self.actual = actual.to_string();
        self.expected = expected.to_string();
        self
    }

    pub fn with_details(mut self, details: &str) -> Self {
        self.details = Some(details.to_string());
        self
    }

    pub fn with_priority(mut self, priority: CheckPriority) -> Self {
        self.priority = priority;
        self
    }

    pub fn is_passed(&self) -> bool {
        matches!(self.status, ValidationStatus::Pass)
    }

    pub fn is_critical_failure(&self) -> bool {
        matches!(self.status, ValidationStatus::Fail)
            && matches!(self.priority, CheckPriority::Critical)
    }
}

/// Priority level for validation checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CheckPriority {
    Critical,
    High,
    Medium,
    Low,
}

/// Complete validation benchmark results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationBenchmarkResults {
    /// Configuration used.
    pub config: ValidationBenchmarkConfig,
    /// Start time.
    pub start_time: chrono::DateTime<chrono::Utc>,
    /// End time.
    pub end_time: chrono::DateTime<chrono::Utc>,
    /// Duration in seconds.
    pub duration_secs: f64,
    /// Dataset information.
    pub dataset_info: ValidationDatasetInfo,
    /// Results by phase.
    pub phase_results: HashMap<ValidationPhase, PhaseResult>,
    /// Overall summary.
    pub summary: ValidationSummary,
    /// Recommendations.
    pub recommendations: Vec<Recommendation>,
}

/// Dataset information for validation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ValidationDatasetInfo {
    /// Total chunks available.
    pub total_chunks: usize,
    /// Chunks used for validation.
    pub chunks_used: usize,
    /// Number of unique topics.
    pub num_topics: usize,
    /// Number of documents.
    pub num_documents: usize,
    /// Dataset loaded successfully.
    pub loaded: bool,
}

/// Result for a single validation phase.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseResult {
    /// Phase name.
    pub phase: ValidationPhase,
    /// Duration in milliseconds.
    pub duration_ms: u64,
    /// Individual checks.
    pub checks: Vec<ValidationCheck>,
    /// Passed count.
    pub passed: usize,
    /// Failed count.
    pub failed: usize,
    /// Warning count.
    pub warnings: usize,
    /// Skipped count.
    pub skipped: usize,
    /// All passed?
    pub all_passed: bool,
}

impl PhaseResult {
    pub fn new(phase: ValidationPhase) -> Self {
        Self {
            phase,
            duration_ms: 0,
            checks: Vec::new(),
            passed: 0,
            failed: 0,
            warnings: 0,
            skipped: 0,
            all_passed: true,
        }
    }

    pub fn add_check(&mut self, check: ValidationCheck) {
        match check.status {
            ValidationStatus::Pass => self.passed += 1,
            ValidationStatus::Fail => {
                self.failed += 1;
                self.all_passed = false;
            }
            ValidationStatus::Warning => self.warnings += 1,
            ValidationStatus::Skip => self.skipped += 1,
        }
        self.checks.push(check);
    }

    pub fn finalize(&mut self, duration_ms: u64) {
        self.duration_ms = duration_ms;
    }
}

/// Overall validation summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationSummary {
    /// Total checks.
    pub total_checks: usize,
    /// Passed checks.
    pub passed_checks: usize,
    /// Failed checks.
    pub failed_checks: usize,
    /// Warning checks.
    pub warning_checks: usize,
    /// Skipped checks.
    pub skipped_checks: usize,
    /// Critical failures.
    pub critical_failures: usize,
    /// All passed?
    pub all_passed: bool,
    /// ARCH rules passed.
    pub arch_rules_passed: usize,
    /// ARCH rules total.
    pub arch_rules_total: usize,
    /// Per-phase summary.
    pub per_phase: HashMap<ValidationPhase, PhaseSummary>,
}

impl Default for ValidationSummary {
    fn default() -> Self {
        Self {
            total_checks: 0,
            passed_checks: 0,
            failed_checks: 0,
            warning_checks: 0,
            skipped_checks: 0,
            critical_failures: 0,
            all_passed: true,
            arch_rules_passed: 0,
            arch_rules_total: 0,
            per_phase: HashMap::new(),
        }
    }
}

/// Summary for a single phase.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseSummary {
    pub passed: usize,
    pub failed: usize,
    pub total: usize,
}

/// Optimization recommendation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recommendation {
    /// Category (Critical, Performance, Parameter).
    pub category: RecommendationCategory,
    /// Description.
    pub description: String,
    /// Affected component.
    pub component: String,
    /// Current value.
    pub current: Option<String>,
    /// Recommended value.
    pub recommended: Option<String>,
    /// Reason.
    pub reason: String,
}

/// Recommendation categories.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecommendationCategory {
    /// Critical issue (must fix).
    Critical,
    /// Performance optimization.
    Performance,
    /// Parameter tuning.
    ParameterTuning,
    /// Best practice.
    BestPractice,
}

/// Report generator for validation results.
pub struct ValidationReportGenerator {
    results: ValidationBenchmarkResults,
}

impl ValidationReportGenerator {
    pub fn new(results: ValidationBenchmarkResults) -> Self {
        Self { results }
    }

    /// Generate markdown report.
    pub fn generate_markdown(&self) -> String {
        let mut report = String::new();

        // Header
        report.push_str("# Unified Real Data Benchmark Validation Report\n\n");
        report.push_str(&format!(
            "Generated: {}\n\n",
            self.results.end_time.format("%Y-%m-%d %H:%M:%S UTC")
        ));

        // Summary
        report.push_str("## Summary\n\n");
        let summary = &self.results.summary;
        report.push_str("| Metric | Value |\n");
        report.push_str("|--------|-------|\n");
        report.push_str(&format!("| Total Checks | {} |\n", summary.total_checks));
        report.push_str(&format!("| Passed | {} |\n", summary.passed_checks));
        report.push_str(&format!("| Failed | {} |\n", summary.failed_checks));
        report.push_str(&format!("| Warnings | {} |\n", summary.warning_checks));
        report.push_str(&format!(
            "| Critical Failures | {} |\n",
            summary.critical_failures
        ));
        report.push_str(&format!(
            "| Overall Status | {} |\n\n",
            if summary.all_passed { "PASS" } else { "FAIL" }
        ));

        // ARCH Compliance
        report.push_str("## ARCH Compliance\n\n");
        report.push_str(&format!(
            "Rules Passed: {}/{}\n\n",
            summary.arch_rules_passed, summary.arch_rules_total
        ));

        // Phase Results
        report.push_str("## Phase Results\n\n");
        for (phase, result) in &self.results.phase_results {
            report.push_str(&format!("### {}\n\n", phase.as_str()));
            report.push_str(&format!(
                "Duration: {}ms | Passed: {} | Failed: {} | Warnings: {}\n\n",
                result.duration_ms, result.passed, result.failed, result.warnings
            ));

            if !result.checks.is_empty() {
                report.push_str("| Check | Status | Expected | Actual |\n");
                report.push_str("|-------|--------|----------|--------|\n");
                for check in &result.checks {
                    let status = match check.status {
                        ValidationStatus::Pass => "PASS",
                        ValidationStatus::Fail => "FAIL",
                        ValidationStatus::Warning => "WARN",
                        ValidationStatus::Skip => "SKIP",
                    };
                    report.push_str(&format!(
                        "| {} | {} | {} | {} |\n",
                        check.name, status, check.expected, check.actual
                    ));
                }
                report.push('\n');
            }
        }

        // Recommendations
        if !self.results.recommendations.is_empty() {
            report.push_str("## Recommendations\n\n");

            // Critical
            let critical: Vec<_> = self
                .results
                .recommendations
                .iter()
                .filter(|r| matches!(r.category, RecommendationCategory::Critical))
                .collect();
            if !critical.is_empty() {
                report.push_str("### Critical Issues (Must Fix)\n\n");
                for (i, rec) in critical.iter().enumerate() {
                    report.push_str(&format!(
                        "{}. [{}] {}\n",
                        i + 1,
                        rec.component,
                        rec.description
                    ));
                }
                report.push('\n');
            }

            // Performance
            let perf: Vec<_> = self
                .results
                .recommendations
                .iter()
                .filter(|r| matches!(r.category, RecommendationCategory::Performance))
                .collect();
            if !perf.is_empty() {
                report.push_str("### Performance Optimizations\n\n");
                for (i, rec) in perf.iter().enumerate() {
                    report.push_str(&format!("{}. {}\n", i + 1, rec.description));
                }
                report.push('\n');
            }

            // Parameter Tuning
            let params: Vec<_> = self
                .results
                .recommendations
                .iter()
                .filter(|r| matches!(r.category, RecommendationCategory::ParameterTuning))
                .collect();
            if !params.is_empty() {
                report.push_str("### Parameter Tuning\n\n");
                report.push_str("| Parameter | Current | Recommended | Reason |\n");
                report.push_str("|-----------|---------|-------------|--------|\n");
                for rec in params {
                    report.push_str(&format!(
                        "| {} | {} | {} | {} |\n",
                        rec.component,
                        rec.current.as_deref().unwrap_or("-"),
                        rec.recommended.as_deref().unwrap_or("-"),
                        rec.reason
                    ));
                }
                report.push('\n');
            }
        }

        report
    }

    /// Write report to file.
    pub fn write_to_file(&self, path: &std::path::Path) -> std::io::Result<()> {
        std::fs::write(path, self.generate_markdown())
    }
}

/// Generate console summary for validation results.
pub fn generate_console_summary(results: &ValidationBenchmarkResults) -> String {
    let mut s = String::new();
    let summary = &results.summary;

    s.push_str("\n╔══════════════════════════════════════════════════════════════════╗\n");
    s.push_str("║           VALIDATION BENCHMARK SUMMARY                           ║\n");
    s.push_str("╠══════════════════════════════════════════════════════════════════╣\n");

    // Overall status
    let status = if summary.all_passed { "PASS" } else { "FAIL" };
    s.push_str(&format!("║  Overall Status: {:>47} ║\n", status));
    s.push_str(&format!(
        "║  Total Checks: {:>49} ║\n",
        summary.total_checks
    ));
    s.push_str(&format!("║  Passed: {:>55} ║\n", summary.passed_checks));
    s.push_str(&format!("║  Failed: {:>55} ║\n", summary.failed_checks));
    s.push_str(&format!("║  Warnings: {:>53} ║\n", summary.warning_checks));
    s.push_str(&format!(
        "║  Critical Failures: {:>44} ║\n",
        summary.critical_failures
    ));
    s.push_str("╠══════════════════════════════════════════════════════════════════╣\n");

    // ARCH compliance
    s.push_str(&format!(
        "║  ARCH Rules: {}/{} {:>47} ║\n",
        summary.arch_rules_passed,
        summary.arch_rules_total,
        if summary.arch_rules_passed == summary.arch_rules_total {
            "PASS"
        } else {
            "FAIL"
        }
    ));

    // Per-phase summary
    s.push_str("╠══════════════════════════════════════════════════════════════════╣\n");
    for phase in ValidationPhase::all() {
        if let Some(result) = results.phase_results.get(&phase) {
            let phase_status = if result.all_passed { "PASS" } else { "FAIL" };
            s.push_str(&format!(
                "║  {:20} {:>4}/{:>4} {:>29} ║\n",
                phase.as_str(),
                result.passed,
                result.passed + result.failed,
                phase_status
            ));
        }
    }

    s.push_str("╚══════════════════════════════════════════════════════════════════╝\n");

    s
}

#[cfg(all(test, feature = "benchmark-tests"))]
mod tests {
    use super::*;

    #[test]
    fn test_validation_phase_parse() {
        assert_eq!(
            ValidationPhase::parse_from("config"),
            Some(ValidationPhase::Config)
        );
        assert_eq!(
            ValidationPhase::parse_from("ARCH"),
            Some(ValidationPhase::Arch)
        );
        assert_eq!(ValidationPhase::parse_from("invalid"), None);
    }

    #[test]
    fn test_validation_check() {
        let check = ValidationCheck::new("test", "Test check")
            .pass("1", "1")
            .with_priority(CheckPriority::Critical);
        assert!(check.is_passed());
        assert!(!check.is_critical_failure());

        let failed = ValidationCheck::new("test", "Test check")
            .fail("0", "1")
            .with_priority(CheckPriority::Critical);
        assert!(failed.is_critical_failure());
    }

    #[test]
    fn test_phase_result() {
        let mut result = PhaseResult::new(ValidationPhase::Config);
        result.add_check(ValidationCheck::new("test1", "desc").pass("1", "1"));
        result.add_check(ValidationCheck::new("test2", "desc").fail("0", "1"));
        assert_eq!(result.passed, 1);
        assert_eq!(result.failed, 1);
        assert!(!result.all_passed);
    }
}
