//! Comparison of benchmark results across runs.
//!
//! Loads saved JSON results and generates delta reports for tracking
//! improvements from fine-tuning.

use super::metrics::{FullBenchmarkReport, PhaseBenchmarkResult};
use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// Comparison between two benchmark runs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparisonReport {
    pub baseline_model: String,
    pub baseline_timestamp: String,
    pub tuned_model: String,
    pub tuned_timestamp: String,
    pub phase_comparisons: Vec<PhaseComparison>,
    pub summary: ComparisonSummary,
}

/// Per-phase comparison.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseComparison {
    pub phase: u8,
    pub phase_name: String,
    pub baseline_pass: bool,
    pub tuned_pass: bool,
    pub metric_deltas: HashMap<String, MetricDelta>,
    pub improved: bool,
    pub regressed: bool,
}

/// Delta for a single metric.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricDelta {
    pub baseline: f64,
    pub tuned: f64,
    pub delta: f64,
    pub delta_pct: f64,
    pub improved: bool,
}

/// Overall comparison summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparisonSummary {
    pub baseline_pass_count: usize,
    pub tuned_pass_count: usize,
    pub phases_improved: usize,
    pub phases_regressed: usize,
    pub phases_unchanged: usize,
    pub total_phases: usize,
}

/// Load a benchmark report from JSON file.
pub fn load_results(path: &Path) -> anyhow::Result<FullBenchmarkReport> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("Failed to read {}: {}", path.display(), e))?;
    let report: FullBenchmarkReport = serde_json::from_str(&content)
        .map_err(|e| anyhow::anyhow!("Failed to parse {}: {}", path.display(), e))?;
    Ok(report)
}

/// Compare two benchmark reports and generate delta analysis.
pub fn compare_reports(
    baseline: &FullBenchmarkReport,
    tuned: &FullBenchmarkReport,
) -> ComparisonReport {
    let mut phase_comparisons = Vec::new();
    let mut phases_improved = 0usize;
    let mut phases_regressed = 0usize;

    for baseline_phase in &baseline.phases {
        let tuned_phase = tuned
            .phases
            .iter()
            .find(|p| p.phase == baseline_phase.phase);

        if let Some(tuned_phase) = tuned_phase {
            let comparison = compare_phases(baseline_phase, tuned_phase);
            if comparison.improved {
                phases_improved += 1;
            }
            if comparison.regressed {
                phases_regressed += 1;
            }
            phase_comparisons.push(comparison);
        }
    }

    let phases_unchanged = phase_comparisons.len() - phases_improved - phases_regressed;

    ComparisonReport {
        baseline_model: baseline.model_name.clone(),
        baseline_timestamp: baseline.timestamp.clone(),
        tuned_model: tuned.model_name.clone(),
        tuned_timestamp: tuned.timestamp.clone(),
        phase_comparisons,
        summary: ComparisonSummary {
            baseline_pass_count: baseline.count_pass(),
            tuned_pass_count: tuned.count_pass(),
            phases_improved,
            phases_regressed,
            phases_unchanged,
            total_phases: baseline.phases.len(),
        },
    }
}

fn compare_phases(
    baseline: &PhaseBenchmarkResult,
    tuned: &PhaseBenchmarkResult,
) -> PhaseComparison {
    let mut metric_deltas = HashMap::new();
    let mut any_improved = false;
    let mut any_regressed = false;

    for (key, &baseline_val) in &baseline.metrics {
        if let Some(&tuned_val) = tuned.metrics.get(key) {
            let delta = tuned_val - baseline_val;
            let delta_pct = if baseline_val.abs() > 1e-8 {
                delta / baseline_val * 100.0
            } else {
                0.0
            };

            // Determine if improvement depends on metric direction
            let is_inverse = key.ends_with("_max")
                || key.contains("anisotropy")
                || key.contains("overhead")
                || key.contains("gap")
                || key.contains("fp");

            let improved = if is_inverse {
                tuned_val < baseline_val
            } else {
                tuned_val > baseline_val
            };

            if improved && delta.abs() > 0.001 {
                any_improved = true;
            }
            if !improved && delta.abs() > 0.001 {
                any_regressed = true;
            }

            metric_deltas.insert(
                key.clone(),
                MetricDelta {
                    baseline: baseline_val,
                    tuned: tuned_val,
                    delta,
                    delta_pct,
                    improved,
                },
            );
        }
    }

    PhaseComparison {
        phase: baseline.phase,
        phase_name: baseline.phase_name.clone(),
        baseline_pass: baseline.pass,
        tuned_pass: tuned.pass,
        metric_deltas,
        improved: any_improved && !any_regressed,
        regressed: any_regressed && !any_improved,
    }
}

/// Generate markdown comparison report.
pub fn generate_comparison_markdown(comparison: &ComparisonReport) -> String {
    let mut out = String::new();
    out.push_str("# Causal Embedding Benchmark Comparison\n\n");
    out.push_str(&format!(
        "**Baseline**: {} ({})\n",
        comparison.baseline_model, comparison.baseline_timestamp
    ));
    out.push_str(&format!(
        "**Tuned**: {} ({})\n\n",
        comparison.tuned_model, comparison.tuned_timestamp
    ));

    out.push_str("## Summary\n\n");
    out.push_str(&format!(
        "- Pass count: {} → {} ({:+})\n",
        comparison.summary.baseline_pass_count,
        comparison.summary.tuned_pass_count,
        comparison.summary.tuned_pass_count as i32 - comparison.summary.baseline_pass_count as i32
    ));
    out.push_str(&format!(
        "- Phases improved: {}\n",
        comparison.summary.phases_improved
    ));
    out.push_str(&format!(
        "- Phases regressed: {}\n",
        comparison.summary.phases_regressed
    ));
    out.push_str(&format!(
        "- Phases unchanged: {}\n\n",
        comparison.summary.phases_unchanged
    ));

    out.push_str("## Phase Details\n\n");
    out.push_str("| Phase | Name | Baseline | Tuned | Trend |\n");
    out.push_str("|-------|------|----------|-------|-------|\n");

    for pc in &comparison.phase_comparisons {
        let baseline_status = if pc.baseline_pass { "PASS" } else { "FAIL" };
        let tuned_status = if pc.tuned_pass { "PASS" } else { "FAIL" };
        let trend = if pc.improved {
            "UP"
        } else if pc.regressed {
            "DOWN"
        } else {
            "---"
        };
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} |\n",
            pc.phase, pc.phase_name, baseline_status, tuned_status, trend
        ));
    }

    out.push_str("\n## Metric Deltas\n\n");
    for pc in &comparison.phase_comparisons {
        if pc.metric_deltas.is_empty() {
            continue;
        }
        out.push_str(&format!("### Phase {}: {}\n\n", pc.phase, pc.phase_name));
        out.push_str("| Metric | Baseline | Tuned | Delta | % Change |\n");
        out.push_str("|--------|----------|-------|-------|----------|\n");

        let mut sorted_keys: Vec<_> = pc.metric_deltas.keys().collect();
        sorted_keys.sort();

        for key in sorted_keys {
            let md = &pc.metric_deltas[key];
            let sign = if md.delta >= 0.0 { "+" } else { "" };
            out.push_str(&format!(
                "| {} | {:.4} | {:.4} | {}{:.4} | {}{:.1}% |\n",
                key, md.baseline, md.tuned, sign, md.delta, sign, md.delta_pct
            ));
        }
        out.push('\n');
    }

    out
}

/// Print comparison to console.
pub fn print_comparison(comparison: &ComparisonReport) {
    println!();
    println!("=== Benchmark Comparison ===");
    println!(
        "Baseline: {} ({})",
        comparison.baseline_model, comparison.baseline_timestamp
    );
    println!(
        "Tuned:    {} ({})",
        comparison.tuned_model, comparison.tuned_timestamp
    );
    println!();

    for pc in &comparison.phase_comparisons {
        let baseline = if pc.baseline_pass { "PASS" } else { "FAIL" };
        let tuned = if pc.tuned_pass { "PASS" } else { "FAIL" };
        let trend = if pc.improved {
            " [IMPROVED]"
        } else if pc.regressed {
            " [REGRESSED]"
        } else {
            ""
        };
        println!(
            "Phase {} [{}]: {} -> {}{}",
            pc.phase, pc.phase_name, baseline, tuned, trend
        );
    }

    println!();
    println!(
        "Overall: {}/{} -> {}/{} ({:+} phases)",
        comparison.summary.baseline_pass_count,
        comparison.summary.total_phases,
        comparison.summary.tuned_pass_count,
        comparison.summary.total_phases,
        comparison.summary.tuned_pass_count as i32 - comparison.summary.baseline_pass_count as i32
    );
}

#[cfg(all(test, feature = "benchmark-tests"))]
mod tests {
    use super::*;
    use crate::causal_bench::metrics::PhaseBenchmarkResult;

    fn make_report(model: &str, accuracy: f64, pass: bool) -> FullBenchmarkReport {
        let mut metrics = HashMap::new();
        metrics.insert("accuracy".to_string(), accuracy);
        let mut targets = HashMap::new();
        targets.insert("accuracy".to_string(), 0.90);

        FullBenchmarkReport {
            model_name: model.into(),
            timestamp: "2026-02-10".into(),
            phases: vec![PhaseBenchmarkResult {
                phase: 1,
                phase_name: "Test Phase".into(),
                metrics,
                targets,
                pass,
                failing_criteria: if pass {
                    vec![]
                } else {
                    vec!["accuracy".into()]
                },
                duration_ms: 100,
                model_dependent: true,
            }],
            overall_pass_count: if pass { 1 } else { 0 },
            overall_total: 1,
            model_pass_count: if pass { 1 } else { 0 },
            model_total: 1,
        }
    }

    #[test]
    fn test_compare_reports() {
        let baseline = make_report("baseline", 0.75, false);
        let tuned = make_report("tuned", 0.95, true);
        let comparison = compare_reports(&baseline, &tuned);

        assert_eq!(comparison.summary.baseline_pass_count, 0);
        assert_eq!(comparison.summary.tuned_pass_count, 1);
        assert_eq!(comparison.summary.phases_improved, 1);
    }

    #[test]
    fn test_comparison_markdown() {
        let baseline = make_report("baseline", 0.75, false);
        let tuned = make_report("tuned", 0.95, true);
        let comparison = compare_reports(&baseline, &tuned);
        let md = generate_comparison_markdown(&comparison);
        assert!(md.contains("Benchmark Comparison"));
        assert!(md.contains("PASS"));
    }
}
