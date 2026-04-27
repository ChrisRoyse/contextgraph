//! Report generation for causal embedding benchmarks.
//!
//! Provides JSON serialization, markdown formatting, and console summary output.

use super::metrics::{FullBenchmarkReport, PhaseBenchmarkResult};
use std::path::Path;

/// Save benchmark report as JSON.
pub fn save_json(report: &FullBenchmarkReport, path: &Path) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(report)?;
    std::fs::write(path, json)?;
    tracing::info!("Saved JSON report to {}", path.display());
    Ok(())
}

/// Save benchmark report as markdown.
pub fn save_markdown(report: &FullBenchmarkReport, path: &Path) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let md = generate_markdown(report);
    std::fs::write(path, md)?;
    tracing::info!("Saved markdown report to {}", path.display());
    Ok(())
}

/// Generate markdown report string.
pub fn generate_markdown(report: &FullBenchmarkReport) -> String {
    let mut out = String::new();
    out.push_str("# Causal Embedding Benchmark Report\n\n");
    out.push_str(&format!("**Model**: {}\n", report.model_name));
    out.push_str(&format!("**Date**: {}\n", report.timestamp));
    out.push_str(&format!(
        "**Overall**: {}/{} PASS\n",
        report.count_pass(),
        report.overall_total
    ));
    let model_total = report.count_model_total();
    if model_total > 0 {
        out.push_str(&format!(
            "**Model quality**: {}/{} PASS  |  **Infrastructure**: {}/{} PASS\n",
            report.count_model_pass(),
            model_total,
            report.count_infra_pass(),
            report.count_infra_total(),
        ));
    }
    out.push('\n');

    out.push_str("## Phase Results\n\n");
    out.push_str("| Phase | Name | Status | Key Metrics |\n");
    out.push_str("|-------|------|--------|-------------|\n");

    for phase in &report.phases {
        let status = phase_status(phase);
        let key_metrics = format_key_metrics(phase);
        out.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            phase.phase, phase.phase_name, status, key_metrics
        ));
    }

    out.push_str("\n## Detailed Metrics\n\n");
    for phase in &report.phases {
        out.push_str(&format!(
            "### Phase {}: {}\n\n",
            phase.phase, phase.phase_name
        ));
        out.push_str(&format!("Duration: {}ms\n\n", phase.duration_ms));

        out.push_str("| Metric | Value | Target | Status |\n");
        out.push_str("|--------|-------|--------|--------|\n");

        for (key, &value) in &phase.metrics {
            let target = phase.targets.get(key).copied();
            let status = if let Some(t) = target {
                let is_inverse = key.ends_with("_max")
                    || key.contains("anisotropy")
                    || key.contains("overhead")
                    || key.contains("gap")
                    || key.contains("fp");
                if (is_inverse && value <= t) || (!is_inverse && value >= t) {
                    "PASS"
                } else {
                    "FAIL"
                }
            } else {
                "-"
            };
            out.push_str(&format!(
                "| {} | {:.4} | {} | {} |\n",
                key,
                value,
                target.map(|t| format!("{:.4}", t)).unwrap_or("-".into()),
                status
            ));
        }
        out.push('\n');

        if !phase.failing_criteria.is_empty() {
            out.push_str("**Failing criteria:**\n");
            for fc in &phase.failing_criteria {
                out.push_str(&format!("- {}\n", fc));
            }
            out.push('\n');
        }
    }

    out
}

/// Print summary to console with pass/fail indicators.
pub fn print_summary(report: &FullBenchmarkReport) {
    println!();
    println!("=== Causal Embedding Benchmark Report ===");
    println!("Model: {}", report.model_name);
    println!("Date:  {}", report.timestamp);
    println!();

    let max_name_len = report
        .phases
        .iter()
        .map(|p| p.phase_name.len())
        .max()
        .unwrap_or(20);

    for phase in &report.phases {
        let status = phase_status(phase);
        let key = format_key_metrics(phase);
        println!(
            "Phase {} [{:<width$}]:  {}  ({})",
            phase.phase,
            phase.phase_name,
            status,
            key,
            width = max_name_len
        );
    }

    println!();
    println!(
        "Overall: {}/{} PASS, {}/{} WARN, {}/{} FAIL",
        report.count_pass(),
        report.overall_total,
        report.count_warn(),
        report.overall_total,
        report.count_fail(),
        report.overall_total,
    );
    let model_total = report.count_model_total();
    let infra_total = report.count_infra_total();
    if model_total > 0 || infra_total > 0 {
        println!(
            "Model quality: {}/{} PASS  |  Infrastructure: {}/{} PASS",
            report.count_model_pass(),
            model_total,
            report.count_infra_pass(),
            infra_total,
        );
    }
    println!();
}

fn phase_status(phase: &PhaseBenchmarkResult) -> &'static str {
    if phase.pass {
        "PASS"
    } else if phase.failing_criteria.len() < phase.targets.len() {
        "WARN"
    } else {
        "FAIL"
    }
}

fn format_key_metrics(phase: &PhaseBenchmarkResult) -> String {
    // Show up to 3 most important metrics
    let mut parts = Vec::new();
    let priority_keys = [
        "accuracy",
        "top1_accuracy",
        "mrr",
        "tpr",
        "tnr",
        "spread",
        "ratio",
        "delta",
        "overhead",
        "throughput",
        "held_out_accuracy",
        "negation_fp",
    ];

    for key in &priority_keys {
        if let Some(&val) = phase.metrics.get(*key) {
            parts.push(format!("{}={:.3}", key, val));
            if parts.len() >= 3 {
                break;
            }
        }
    }

    // If we found nothing with priority keys, show first 3
    if parts.is_empty() {
        for (key, val) in phase.metrics.iter().take(3) {
            parts.push(format!("{}={:.3}", key, val));
        }
    }

    parts.join(", ")
}

#[cfg(all(test, feature = "benchmark-tests"))]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn sample_report() -> FullBenchmarkReport {
        FullBenchmarkReport {
            model_name: "nomic-embed-text-v1.5".into(),
            timestamp: "2026-02-10".into(),
            phases: vec![
                PhaseBenchmarkResult {
                    phase: 1,
                    phase_name: "Query Intent".into(),
                    metrics: {
                        let mut m = HashMap::new();
                        m.insert("accuracy".into(), 0.96);
                        m
                    },
                    targets: {
                        let mut t = HashMap::new();
                        t.insert("accuracy".into(), 0.90);
                        t
                    },
                    pass: true,
                    failing_criteria: vec![],
                    duration_ms: 50,
                    model_dependent: false,
                },
                PhaseBenchmarkResult {
                    phase: 2,
                    phase_name: "E5 Quality".into(),
                    metrics: {
                        let mut m = HashMap::new();
                        m.insert("spread".into(), 0.026);
                        m
                    },
                    targets: {
                        let mut t = HashMap::new();
                        t.insert("spread".into(), 0.10);
                        t
                    },
                    pass: false,
                    failing_criteria: vec!["spread: 0.026 (target: >=0.10)".into()],
                    duration_ms: 200,
                    model_dependent: true,
                },
            ],
            overall_pass_count: 1,
            overall_total: 2,
            model_pass_count: 0,
            model_total: 1,
        }
    }

    #[test]
    fn test_generate_markdown() {
        let report = sample_report();
        let md = generate_markdown(&report);
        assert!(md.contains("Causal Embedding Benchmark Report"));
        assert!(md.contains("nomic-embed-text-v1.5"));
        assert!(md.contains("Query Intent"));
        assert!(md.contains("PASS"));
    }

    #[test]
    fn test_print_summary() {
        let report = sample_report();
        // Just verify it doesn't panic
        print_summary(&report);
    }
}
