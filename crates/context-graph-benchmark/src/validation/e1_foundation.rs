//! E1 foundation validation.
//!
//! Tests:
//! - E1 is default entry point
//! - E1 provides strong baseline (MRR@10 > 0.6)
//! - ARCH-17: Enhancement behavior (refine when strong, broaden when weak)

use super::{
    CheckPriority, PhaseResult, Recommendation, RecommendationCategory, ValidationCheck,
    ValidationPhase,
};
use crate::realdata::config::{EmbedderName, FusionStrategy};

/// E1 foundation validation results.
#[derive(Debug, Clone)]
pub struct E1FoundationResult {
    /// All checks passed.
    pub all_passed: bool,
    /// Individual check results.
    pub checks: Vec<ValidationCheck>,
    /// E1 baseline metrics.
    pub baseline_metrics: E1BaselineMetrics,
    /// Recommendations.
    pub recommendations: Vec<Recommendation>,
}

/// E1 baseline metrics.
#[derive(Debug, Clone, Default)]
pub struct E1BaselineMetrics {
    /// E1 MRR@10.
    pub mrr_at_10: f64,
    /// E1 Precision@10.
    pub precision_at_10: f64,
    /// E1 Recall@20.
    pub recall_at_20: f64,
    /// Percentage of queries with strong matches (sim > 0.8).
    pub strong_match_percentage: f64,
    /// Percentage of queries with weak matches (sim < 0.4).
    pub weak_match_percentage: f64,
}

/// Thresholds for E1 baseline.
pub const E1_MRR_TARGET: f64 = 0.60; // MRR@10 > 0.6
pub const E1_STRONG_THRESHOLD: f64 = 0.80; // sim > 0.8 = strong
pub const E1_WEAK_THRESHOLD: f64 = 0.40; // sim < 0.4 = weak

/// E1 foundation validator.
pub struct E1FoundationValidator;

impl E1FoundationValidator {
    /// Run all E1 foundation validation checks.
    pub fn validate(
        e1_mrr: Option<f64>,
        e1_precision: Option<f64>,
        e1_recall: Option<f64>,
        strong_match_pct: Option<f64>,
        weak_match_pct: Option<f64>,
    ) -> E1FoundationResult {
        let mut result = E1FoundationResult {
            all_passed: true,
            checks: Vec::new(),
            baseline_metrics: E1BaselineMetrics {
                mrr_at_10: e1_mrr.unwrap_or(0.0),
                precision_at_10: e1_precision.unwrap_or(0.0),
                recall_at_20: e1_recall.unwrap_or(0.0),
                strong_match_percentage: strong_match_pct.unwrap_or(0.0),
                weak_match_percentage: weak_match_pct.unwrap_or(0.0),
            },
            recommendations: Vec::new(),
        };

        // Test 1: E1 is in semantic embedders
        let check = Self::test_e1_in_semantic();
        if !check.is_passed() {
            result.all_passed = false;
        }
        result.checks.push(check);

        // Test 2: E1 is entry embedder (ARCH-12)
        let check = Self::test_e1_is_entry_embedder();
        if !check.is_passed() {
            result.all_passed = false;
        }
        result.checks.push(check);

        // Test 3: Default strategy is E1Only
        let check = Self::test_default_is_e1only();
        if !check.is_passed() {
            result.all_passed = false;
        }
        result.checks.push(check);

        // Test 4: E1 has max topic weight
        let check = Self::test_e1_max_weight();
        if !check.is_passed() {
            result.all_passed = false;
        }
        result.checks.push(check);

        // Test 5: E1 used for divergence detection
        let check = Self::test_e1_used_for_divergence();
        if !check.is_passed() {
            result.all_passed = false;
        }
        result.checks.push(check);

        // If we have actual metrics, validate E1 baseline quality
        if let Some(mrr) = e1_mrr {
            let (check, recommendation) = Self::test_e1_baseline_quality(mrr);
            if !check.is_passed() {
                result.all_passed = false;
            }
            result.checks.push(check);

            if let Some(rec) = recommendation {
                result.recommendations.push(rec);
            }
        }

        // If we have match distribution, validate ARCH-17 behavior
        if strong_match_pct.is_some() || weak_match_pct.is_some() {
            let strong = strong_match_pct.unwrap_or(0.0);
            let weak = weak_match_pct.unwrap_or(0.0);
            let check = Self::test_arch_17_enhancement_behavior(strong, weak);
            result.checks.push(check);
        }

        result
    }

    /// Convert to PhaseResult.
    pub fn to_phase_result(result: E1FoundationResult, duration_ms: u64) -> PhaseResult {
        let mut phase_result = PhaseResult::new(ValidationPhase::E1Foundation);
        for check in result.checks {
            phase_result.add_check(check);
        }
        phase_result.finalize(duration_ms);
        phase_result
    }

    // Individual tests

    fn test_e1_in_semantic() -> ValidationCheck {
        let check = ValidationCheck::new("e1_in_semantic", "E1 is in semantic embedders")
            .with_priority(CheckPriority::Critical);

        let semantic = EmbedderName::semantic();
        let e1_in_semantic = semantic.contains(&EmbedderName::E1Semantic);

        if e1_in_semantic {
            check.pass("E1 in semantic", "E1 in semantic")
        } else {
            check.fail("E1 not in semantic", "E1 in semantic")
        }
    }

    fn test_e1_is_entry_embedder() -> ValidationCheck {
        let check = ValidationCheck::new(
            "e1_is_entry_embedder",
            "[ARCH-12] E1 is the default entry point for all retrieval",
        )
        .with_priority(CheckPriority::Critical);

        // E1 should have index 0 (first embedder)
        let e1_index = EmbedderName::E1Semantic.index();

        // Default strategy should be E1Only
        let default_strategy = FusionStrategy::default();

        if e1_index == 0 && default_strategy == FusionStrategy::E1Only {
            check.pass("index=0, default=E1Only", "E1 is entry point")
        } else {
            check.fail(
                &format!("index={}, default={:?}", e1_index, default_strategy),
                "index=0, default=E1Only",
            )
        }
    }

    fn test_default_is_e1only() -> ValidationCheck {
        let check = ValidationCheck::new(
            "default_is_e1only",
            "[ARCH-13] Default fusion strategy is E1Only",
        )
        .with_priority(CheckPriority::Critical);

        let default = FusionStrategy::default();

        if default == FusionStrategy::E1Only {
            check.pass("E1Only", "E1Only")
        } else {
            check.fail(&format!("{:?}", default), "E1Only")
        }
    }

    fn test_e1_max_weight() -> ValidationCheck {
        let check = ValidationCheck::new("e1_max_weight", "E1 has maximum topic weight (1.0)")
            .with_priority(CheckPriority::High);

        let e1_weight = EmbedderName::E1Semantic.topic_weight();
        let max_weight = EmbedderName::all()
            .iter()
            .map(|e| e.topic_weight())
            .fold(0.0f64, f64::max);

        if (e1_weight - max_weight).abs() < f64::EPSILON && (e1_weight - 1.0).abs() < f64::EPSILON {
            check.pass("1.0", "1.0 (max)")
        } else {
            check.fail(&format!("{}", e1_weight), "1.0 (max)")
        }
    }

    fn test_e1_used_for_divergence() -> ValidationCheck {
        let check = ValidationCheck::new(
            "e1_used_for_divergence",
            "[ARCH-10] E1 used for divergence detection",
        )
        .with_priority(CheckPriority::High);

        let used = EmbedderName::E1Semantic.used_for_divergence();

        if used {
            check.pass("E1 in divergence", "E1 used")
        } else {
            check.fail("E1 not in divergence", "E1 used")
        }
    }

    fn test_e1_baseline_quality(mrr_at_10: f64) -> (ValidationCheck, Option<Recommendation>) {
        let check = ValidationCheck::new(
            "e1_baseline_quality",
            "E1 provides strong baseline (MRR@10 > 0.6)",
        )
        .with_priority(CheckPriority::High);

        let meets_target = mrr_at_10 >= E1_MRR_TARGET;

        if meets_target {
            (
                check.pass(
                    &format!("{:.3}", mrr_at_10),
                    &format!("> {:.1}", E1_MRR_TARGET),
                ),
                None,
            )
        } else {
            let rec = Recommendation {
                category: RecommendationCategory::Critical,
                description: format!("E1 MRR {:.3} below {:.1} target", mrr_at_10, E1_MRR_TARGET),
                component: "E1_Semantic".to_string(),
                current: Some(format!("{:.3}", mrr_at_10)),
                recommended: Some(format!("> {:.1}", E1_MRR_TARGET)),
                reason: "E1 as foundation should provide strong baseline".to_string(),
            };
            (
                check.warning(
                    &format!("{:.3}", mrr_at_10),
                    &format!("> {:.1}", E1_MRR_TARGET),
                ),
                Some(rec),
            )
        }
    }

    fn test_arch_17_enhancement_behavior(strong_pct: f64, weak_pct: f64) -> ValidationCheck {
        let check = ValidationCheck::new(
            "arch_17_enhancement",
            "[ARCH-17] E1 strong (>0.8): enhancers refine; weak (<0.4): enhancers broaden",
        )
        .with_priority(CheckPriority::Medium);

        // This is informational - shows the distribution
        // Strong matches should be refined, weak should be broadened
        let balanced = (strong_pct + weak_pct) < 1.0; // Should not be all edge cases

        if balanced {
            check
                .pass(
                    &format!(
                        "strong={:.0}%, weak={:.0}%",
                        strong_pct * 100.0,
                        weak_pct * 100.0
                    ),
                    "balanced distribution",
                )
                .with_details("Enhancement strategy depends on E1 match strength")
        } else {
            check.warning(
                &format!(
                    "strong={:.0}%, weak={:.0}%",
                    strong_pct * 100.0,
                    weak_pct * 100.0
                ),
                "balanced distribution",
            )
        }
    }

    /// Analyze E1 match strength distribution.
    pub fn analyze_match_distribution(similarities: &[f64]) -> (f64, f64) {
        if similarities.is_empty() {
            return (0.0, 0.0);
        }

        let strong_count = similarities
            .iter()
            .filter(|&&s| s > E1_STRONG_THRESHOLD)
            .count();
        let weak_count = similarities
            .iter()
            .filter(|&&s| s < E1_WEAK_THRESHOLD)
            .count();
        let total = similarities.len() as f64;

        (strong_count as f64 / total, weak_count as f64 / total)
    }
}

#[cfg(all(test, feature = "benchmark-tests"))]
mod tests {
    use super::*;

    #[test]
    fn test_e1_foundation_config() {
        let result = E1FoundationValidator::validate(None, None, None, None, None);
        assert!(result.all_passed);
        assert!(!result.checks.is_empty());
    }

    #[test]
    fn test_e1_in_semantic() {
        let check = E1FoundationValidator::test_e1_in_semantic();
        assert!(check.is_passed());
    }

    #[test]
    fn test_e1_is_entry() {
        let check = E1FoundationValidator::test_e1_is_entry_embedder();
        assert!(check.is_passed());
    }

    #[test]
    fn test_baseline_quality_good() {
        let (check, rec) = E1FoundationValidator::test_e1_baseline_quality(0.72);
        assert!(check.is_passed());
        assert!(rec.is_none());
    }

    #[test]
    fn test_baseline_quality_low() {
        let (check, rec) = E1FoundationValidator::test_e1_baseline_quality(0.45);
        assert!(
            !check.is_passed() || matches!(check.status, super::super::ValidationStatus::Warning)
        );
        assert!(rec.is_some());
    }

    #[test]
    fn test_match_distribution() {
        let sims = vec![0.9, 0.85, 0.5, 0.3, 0.2, 0.7, 0.88, 0.45];
        let (strong, weak) = E1FoundationValidator::analyze_match_distribution(&sims);

        // 0.9, 0.85, 0.88 are > 0.8 (3/8 = 37.5%)
        // 0.3, 0.2 are < 0.4 (2/8 = 25%)
        assert!((strong - 0.375).abs() < 0.01);
        assert!((weak - 0.25).abs() < 0.01);
    }
}
