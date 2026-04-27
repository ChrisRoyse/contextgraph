//! Ablation validation for contribution analysis.
//!
//! Tests:
//! - E1 removal causes significant degradation (>10% MRR drop)
//! - Enhancement embedders improve E1 (>5% improvement)
//! - Temporal excluded from topic detection
//! - Critical vs redundant embedders identified

use std::collections::HashMap;

use super::results_aggregator::AggregatedResults;
use super::{
    CheckPriority, PhaseResult, Recommendation, RecommendationCategory, ValidationCheck,
    ValidationPhase,
};
use crate::realdata::config::EmbedderName;
use crate::realdata::results::AblationResults;

/// Ablation validation results.
#[derive(Debug, Clone)]
pub struct AblationValidationResult {
    /// All checks passed.
    pub all_passed: bool,
    /// Individual check results.
    pub checks: Vec<ValidationCheck>,
    /// Per-embedder impact.
    pub embedder_impact: HashMap<EmbedderName, EmbedderImpact>,
    /// Critical embedders (removal causes >10% degradation).
    pub critical_embedders: Vec<EmbedderName>,
    /// Redundant embedders (removal causes <2% change).
    pub redundant_embedders: Vec<EmbedderName>,
    /// Recommendations.
    pub recommendations: Vec<Recommendation>,
    /// Measured contribution data from benchmark results.
    pub measured_contributions: Option<MeasuredContributions>,
}

/// Measured contribution data from benchmark results.
#[derive(Debug, Clone, Default)]
pub struct MeasuredContributions {
    /// E5 causal contribution percentage.
    pub e5_contribution: Option<f64>,
    /// E7 code contribution percentage.
    pub e7_contribution: Option<f64>,
    /// E8 graph contribution percentage.
    pub e8_contribution: Option<f64>,
    /// E10 multimodal contribution percentage.
    pub e10_contribution: Option<f64>,
}

/// Impact metrics for a single embedder.
#[derive(Debug, Clone, Default)]
pub struct EmbedderImpact {
    /// MRR change when removed (negative = degradation).
    pub removal_mrr_change: f64,
    /// MRR change when added to E1.
    pub addition_mrr_change: f64,
    /// Is this a critical embedder?
    pub is_critical: bool,
    /// Is this a redundant embedder?
    pub is_redundant: bool,
}

/// Thresholds for ablation analysis.
pub const CRITICAL_DEGRADATION_THRESHOLD: f64 = 0.10; // >10% MRR drop
pub const REDUNDANT_CHANGE_THRESHOLD: f64 = 0.02; // <2% change
pub const ENHANCEMENT_TARGET: f64 = 0.05; // >5% improvement expected

/// Ablation validator.
pub struct AblationValidator;

impl AblationValidator {
    /// Run all ablation validation checks.
    pub fn validate(
        ablation_results: Option<&AblationResults>,
        aggregated: Option<&AggregatedResults>,
    ) -> AblationValidationResult {
        // Extract measured contributions from aggregated results
        let measured = aggregated.map(|agg| {
            let mut contributions = MeasuredContributions::default();
            if let Some(causal) = &agg.causal {
                // Use e5_asymmetric_accuracy as the contribution metric (how well E5 performs)
                contributions.e5_contribution = Some(causal.e5_asymmetric_accuracy);
            }
            if let Some(graph) = &agg.graph {
                contributions.e8_contribution = Some(graph.e8_contribution);
            }
            if let Some(multimodal) = &agg.multimodal {
                contributions.e10_contribution = Some(multimodal.e10_contribution_pct);
            }
            contributions
        });

        let mut result = AblationValidationResult {
            all_passed: true,
            checks: Vec::new(),
            embedder_impact: HashMap::new(),
            critical_embedders: Vec::new(),
            redundant_embedders: Vec::new(),
            recommendations: Vec::new(),
            measured_contributions: measured.clone(),
        };

        // Test 1: E1 foundation check (design)
        let check = Self::test_e1_is_foundation();
        if !check.is_passed() {
            result.all_passed = false;
        }
        result.checks.push(check);

        // Test 2: Temporal excluded from topic detection
        let check = Self::test_temporal_excluded();
        if !check.is_passed() {
            result.all_passed = false;
        }
        result.checks.push(check);

        // Test 3: Enhancement embedders defined
        let check = Self::test_enhancement_embedders_defined();
        if !check.is_passed() {
            result.all_passed = false;
        }
        result.checks.push(check);

        // If we have actual ablation results, validate them
        if let Some(ablation) = ablation_results {
            // Test: E1 removal is critical
            if let Some(e1_impact) = ablation.removal_impact.get(&EmbedderName::E1Semantic) {
                let check = Self::test_e1_removal_critical(e1_impact.mrr_change);
                if !check.is_passed() {
                    result.all_passed = false;
                }
                result.checks.push(check);
            }

            // Compute impact for each embedder
            for (embedder, impact) in &ablation.removal_impact {
                let mut emb_impact = EmbedderImpact {
                    removal_mrr_change: impact.mrr_change,
                    ..Default::default()
                };

                if let Some(addition) = ablation.addition_impact.get(embedder) {
                    emb_impact.addition_mrr_change = addition.mrr_change;
                }

                // Classify as critical or redundant
                if impact.mrr_change.abs() > CRITICAL_DEGRADATION_THRESHOLD {
                    emb_impact.is_critical = true;
                    result.critical_embedders.push(*embedder);
                } else if impact.mrr_change.abs() < REDUNDANT_CHANGE_THRESHOLD {
                    emb_impact.is_redundant = true;
                    result.redundant_embedders.push(*embedder);
                }

                result.embedder_impact.insert(*embedder, emb_impact);
            }

            // Test: Enhancement embedders improve E1
            for embedder in &[
                EmbedderName::E5Causal,
                EmbedderName::E7Code,
                EmbedderName::E10Multimodal,
            ] {
                if let Some(addition) = ablation.addition_impact.get(embedder) {
                    let (check, recommendation) =
                        Self::test_enhancement_contribution(*embedder, addition.mrr_change);
                    if !check.is_passed() {
                        result.all_passed = false;
                    }
                    result.checks.push(check);

                    if let Some(rec) = recommendation {
                        result.recommendations.push(rec);
                    }
                }
            }

            // Test: Temporal embedders don't contribute to topic detection
            let temporal = EmbedderName::temporal();
            for embedder in &temporal {
                if let Some(removal) = ablation.removal_impact.get(embedder) {
                    // Temporal removal should have minimal impact on topic-based metrics
                    let check = Self::test_temporal_minimal_impact(*embedder, removal.mrr_change);
                    // This is a warning, not failure
                    result.checks.push(check);
                }
            }
        }

        // Test measured contributions if available
        if let Some(contributions) = &result.measured_contributions {
            // Test E5 causal contribution
            if let Some(e5_contrib) = contributions.e5_contribution {
                let check =
                    Self::test_measured_contribution(EmbedderName::E5Causal, e5_contrib, 0.7);
                result.checks.push(check);
            }

            // Test E8 graph contribution
            if let Some(e8_contrib) = contributions.e8_contribution {
                let check =
                    Self::test_measured_contribution(EmbedderName::E8Graph, e8_contrib, 0.05);
                result.checks.push(check);
            }

            // Test E10 multimodal contribution
            if let Some(e10_contrib) = contributions.e10_contribution {
                let check = Self::test_measured_contribution(
                    EmbedderName::E10Multimodal,
                    e10_contrib,
                    0.05,
                );
                result.checks.push(check);
            }
        }

        result
    }

    /// Convert to PhaseResult.
    pub fn to_phase_result(result: AblationValidationResult, duration_ms: u64) -> PhaseResult {
        let mut phase_result = PhaseResult::new(ValidationPhase::Ablation);
        for check in result.checks {
            phase_result.add_check(check);
        }
        phase_result.finalize(duration_ms);
        phase_result
    }

    // Individual tests

    fn test_e1_is_foundation() -> ValidationCheck {
        let check = ValidationCheck::new(
            "e1_is_foundation",
            "[ARCH-12] E1 is THE semantic foundation",
        )
        .with_priority(CheckPriority::Critical);

        let semantic = EmbedderName::semantic();
        let e1_in_semantic = semantic.contains(&EmbedderName::E1Semantic);
        let e1_has_max_weight = EmbedderName::E1Semantic.topic_weight() == 1.0;

        if e1_in_semantic && e1_has_max_weight {
            check.pass("E1 semantic, weight=1.0", "E1 foundation")
        } else {
            check.fail(
                &format!(
                    "semantic={}, weight={}",
                    e1_in_semantic,
                    EmbedderName::E1Semantic.topic_weight()
                ),
                "E1 foundation",
            )
        }
    }

    fn test_temporal_excluded() -> ValidationCheck {
        let check = ValidationCheck::new(
            "temporal_excluded",
            "[AP-60] Temporal embedders (E2-E4) MUST NOT count toward topic detection",
        )
        .with_priority(CheckPriority::Critical);

        let temporal = EmbedderName::temporal();
        let all_zero = temporal
            .iter()
            .all(|e| e.topic_weight().abs() < f64::EPSILON);

        if all_zero {
            check.pass("E2-E4 weight=0", "temporal excluded")
        } else {
            let non_zero: Vec<_> = temporal
                .iter()
                .filter(|e| e.topic_weight().abs() >= f64::EPSILON)
                .map(|e| format!("{}={}", e.as_str(), e.topic_weight()))
                .collect();
            check.fail(&non_zero.join(", "), "all 0.0")
        }
    }

    fn test_enhancement_embedders_defined() -> ValidationCheck {
        let check = ValidationCheck::new(
            "enhancement_embedders_defined",
            "Enhancement embedders (E5, E7, E10) defined for contribution analysis",
        )
        .with_priority(CheckPriority::High);

        let semantic = EmbedderName::semantic();
        let enhancers = [
            EmbedderName::E5Causal,
            EmbedderName::E7Code,
            EmbedderName::E10Multimodal,
        ];
        let all_defined = enhancers.iter().all(|e| semantic.contains(e));

        if all_defined {
            check.pass("E5, E7, E10 in semantic", "enhancers defined")
        } else {
            check.fail("missing enhancers", "E5, E7, E10")
        }
    }

    fn test_e1_removal_critical(mrr_change: f64) -> ValidationCheck {
        let check = ValidationCheck::new(
            "e1_removal_critical",
            "E1 removal causes >10% MRR degradation",
        )
        .with_priority(CheckPriority::Critical);

        // MRR change should be negative (degradation) and > 10%
        let is_critical = mrr_change < -CRITICAL_DEGRADATION_THRESHOLD;

        if is_critical {
            check.pass(&format!("{:.1}% drop", mrr_change * 100.0), "> 10% drop")
        } else {
            check
                .fail(&format!("{:.1}% change", mrr_change * 100.0), "> 10% drop")
                .with_details("E1 as foundation should be critical")
        }
    }

    fn test_enhancement_contribution(
        embedder: EmbedderName,
        mrr_change: f64,
    ) -> (ValidationCheck, Option<Recommendation>) {
        let check = ValidationCheck::new(
            &format!("{}_enhancement_contribution", embedder.as_str()),
            &format!("{} improves E1 by >5%", embedder.as_str()),
        )
        .with_priority(CheckPriority::High);

        let meets_target = mrr_change >= ENHANCEMENT_TARGET;

        if meets_target {
            (
                check.pass(&format!("{:.1}% improvement", mrr_change * 100.0), "> 5%"),
                None,
            )
        } else {
            let rec = Recommendation {
                category: RecommendationCategory::Performance,
                description: format!(
                    "{} contribution {:.1}% below 5% target",
                    embedder.as_str(),
                    mrr_change * 100.0
                ),
                component: embedder.as_str().to_string(),
                current: Some(format!("{:.1}%", mrr_change * 100.0)),
                recommended: Some("> 5%".to_string()),
                reason: "Enhancement embedders should meaningfully improve E1 baseline".to_string(),
            };
            (
                check.warning(&format!("{:.1}%", mrr_change * 100.0), "> 5%"),
                Some(rec),
            )
        }
    }

    fn test_temporal_minimal_impact(embedder: EmbedderName, mrr_change: f64) -> ValidationCheck {
        let check = ValidationCheck::new(
            &format!("{}_temporal_impact", embedder.as_str()),
            &format!("{} removal has minimal topic impact", embedder.as_str()),
        )
        .with_priority(CheckPriority::Medium);

        // Temporal removal should have minimal impact since they don't contribute to topics
        // We expect |mrr_change| < 5% for topic-based metrics
        let minimal_impact = mrr_change.abs() < 0.05;

        if minimal_impact {
            check.pass(&format!("{:.1}% change", mrr_change * 100.0), "< 5% change")
        } else {
            check
                .warning(&format!("{:.1}% change", mrr_change * 100.0), "< 5% change")
                .with_details(
                    "Temporal embedders shouldn't affect topic-based metrics significantly",
                )
        }
    }

    /// Classify embedders based on ablation impact.
    pub fn classify_embedders(
        removal_impacts: &HashMap<EmbedderName, f64>,
    ) -> (Vec<EmbedderName>, Vec<EmbedderName>) {
        let mut critical = Vec::new();
        let mut redundant = Vec::new();

        for (embedder, &impact) in removal_impacts {
            if impact.abs() > CRITICAL_DEGRADATION_THRESHOLD {
                critical.push(*embedder);
            } else if impact.abs() < REDUNDANT_CHANGE_THRESHOLD {
                redundant.push(*embedder);
            }
        }

        (critical, redundant)
    }

    /// Test measured contribution from benchmark results.
    fn test_measured_contribution(
        embedder: EmbedderName,
        contribution: f64,
        target: f64,
    ) -> ValidationCheck {
        let check = ValidationCheck::new(
            &format!("{}_measured_contribution", embedder.as_str()),
            &format!("{} measured contribution (benchmark)", embedder.as_str()),
        )
        .with_priority(CheckPriority::High);

        if contribution >= target {
            check.pass(
                &format!("{:.1}%", contribution * 100.0),
                &format!(">= {:.0}%", target * 100.0),
            )
        } else if contribution >= target * 0.5 {
            check
                .warning(
                    &format!("{:.1}%", contribution * 100.0),
                    &format!(">= {:.0}%", target * 100.0),
                )
                .with_details("Below target but showing positive contribution")
        } else {
            check.fail(
                &format!("{:.1}%", contribution * 100.0),
                &format!(">= {:.0}%", target * 100.0),
            )
        }
    }
}

#[cfg(all(test, feature = "benchmark-tests"))]
mod tests {
    use super::*;

    #[test]
    fn test_ablation_validation_config() {
        let result = AblationValidator::validate(None, None);
        assert!(result.all_passed);
        assert!(!result.checks.is_empty());
    }

    #[test]
    fn test_e1_foundation() {
        let check = AblationValidator::test_e1_is_foundation();
        assert!(check.is_passed());
    }

    #[test]
    fn test_temporal_excluded() {
        let check = AblationValidator::test_temporal_excluded();
        assert!(check.is_passed());
    }

    #[test]
    fn test_classify_embedders() {
        let mut impacts = HashMap::new();
        impacts.insert(EmbedderName::E1Semantic, -0.15); // Critical
        impacts.insert(EmbedderName::E5Causal, -0.08); // Normal
        impacts.insert(EmbedderName::E9HDC, -0.01); // Redundant

        let (critical, redundant) = AblationValidator::classify_embedders(&impacts);
        assert!(critical.contains(&EmbedderName::E1Semantic));
        assert!(redundant.contains(&EmbedderName::E9HDC));
        assert!(!critical.contains(&EmbedderName::E5Causal));
        assert!(!redundant.contains(&EmbedderName::E5Causal));
    }

    #[test]
    fn test_with_aggregated_results() {
        use super::super::results_aggregator::{
            AggregatedResults, GraphMetrics, MultimodalMetrics,
        };

        let mut agg = AggregatedResults::default();
        agg.graph = Some(GraphMetrics {
            e8_contribution: 0.08,
            ..Default::default()
        });
        agg.multimodal = Some(MultimodalMetrics {
            e10_contribution_pct: 0.07,
            ..Default::default()
        });

        let result = AblationValidator::validate(None, Some(&agg));
        assert!(result.all_passed);
        assert!(result.measured_contributions.is_some());

        let contributions = result.measured_contributions.unwrap();
        assert!(contributions.e8_contribution.is_some());
        assert!(contributions.e10_contribution.is_some());
    }
}
