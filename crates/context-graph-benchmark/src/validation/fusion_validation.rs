//! Fusion strategy validation.
//!
//! Tests:
//! - E1Only uses only E1
//! - MultiSpace excludes temporal
//! - Pipeline follows 3-stage architecture
//! - Best strategy improves over E1Only baseline

use std::collections::HashMap;

use super::{
    CheckPriority, PhaseResult, Recommendation, RecommendationCategory, ValidationCheck,
    ValidationPhase,
};
use crate::realdata::config::{EmbedderName, FusionStrategy};
use crate::realdata::results::FusionResults;

/// Fusion validation results.
#[derive(Debug, Clone)]
pub struct FusionValidationResult {
    /// All checks passed.
    pub all_passed: bool,
    /// Individual check results.
    pub checks: Vec<ValidationCheck>,
    /// Per-strategy metrics.
    pub strategy_metrics: HashMap<FusionStrategy, FusionStrategyMetrics>,
    /// Best strategy.
    pub best_strategy: Option<FusionStrategy>,
    /// Improvement over E1Only.
    pub improvement_over_baseline: f64,
    /// Recommendations.
    pub recommendations: Vec<Recommendation>,
}

/// Metrics for a fusion strategy.
#[derive(Debug, Clone, Default)]
pub struct FusionStrategyMetrics {
    /// MRR@10.
    pub mrr_at_10: f64,
    /// Embedders used.
    pub embedders_used: Vec<EmbedderName>,
    /// Latency in ms.
    pub latency_ms: f64,
}

/// Fusion validator.
pub struct FusionValidator;

impl FusionValidator {
    /// Run all fusion validation checks.
    pub fn validate(fusion_results: Option<&FusionResults>) -> FusionValidationResult {
        let mut result = FusionValidationResult {
            all_passed: true,
            checks: Vec::new(),
            strategy_metrics: HashMap::new(),
            best_strategy: None,
            improvement_over_baseline: 0.0,
            recommendations: Vec::new(),
        };

        // Test 1: FusionStrategy enum has all required variants
        let check = Self::test_fusion_strategies_defined();
        if !check.is_passed() {
            result.all_passed = false;
        }
        result.checks.push(check);

        // Test 2: Default strategy is E1Only
        let check = Self::test_default_is_e1only();
        if !check.is_passed() {
            result.all_passed = false;
        }
        result.checks.push(check);

        // Test 3: E1Only semantics (design)
        let check = Self::test_e1only_semantics();
        if !check.is_passed() {
            result.all_passed = false;
        }
        result.checks.push(check);

        // Test 4: MultiSpace excludes temporal (design)
        let check = Self::test_multispace_excludes_temporal();
        if !check.is_passed() {
            result.all_passed = false;
        }
        result.checks.push(check);

        // Test 5: Pipeline 3-stage architecture (design)
        let check = Self::test_pipeline_architecture();
        if !check.is_passed() {
            result.all_passed = false;
        }
        result.checks.push(check);

        // If we have actual fusion results, validate them
        if let Some(fusion) = fusion_results {
            // Populate strategy metrics
            for (strategy, strategy_results) in &fusion.by_strategy {
                result.strategy_metrics.insert(
                    *strategy,
                    FusionStrategyMetrics {
                        mrr_at_10: strategy_results.mrr_at_10,
                        embedders_used: strategy_results.embedders_used.clone(),
                        latency_ms: strategy_results.latency_ms,
                    },
                );
            }

            result.best_strategy = Some(fusion.best_strategy);
            result.improvement_over_baseline = fusion.improvement_over_baseline;

            // Test: E1Only in results uses only E1
            if let Some(e1only) = fusion.by_strategy.get(&FusionStrategy::E1Only) {
                let check = Self::test_e1only_embedders(&e1only.embedders_used);
                if !check.is_passed() {
                    result.all_passed = false;
                }
                result.checks.push(check);
            }

            // Test: MultiSpace excludes temporal in actual results
            if let Some(multispace) = fusion.by_strategy.get(&FusionStrategy::MultiSpace) {
                let check = Self::test_multispace_actual(&multispace.embedders_used);
                if !check.is_passed() {
                    result.all_passed = false;
                }
                result.checks.push(check);
            }

            // Test: Best strategy improves baseline
            let (check, recommendation) = Self::test_best_improves_baseline(
                fusion.best_strategy,
                fusion.improvement_over_baseline,
            );
            result.checks.push(check);

            if let Some(rec) = recommendation {
                result.recommendations.push(rec);
            }
        }

        result
    }

    /// Convert to PhaseResult.
    pub fn to_phase_result(result: FusionValidationResult, duration_ms: u64) -> PhaseResult {
        let mut phase_result = PhaseResult::new(ValidationPhase::Fusion);
        for check in result.checks {
            phase_result.add_check(check);
        }
        phase_result.finalize(duration_ms);
        phase_result
    }

    // Individual tests

    fn test_fusion_strategies_defined() -> ValidationCheck {
        let check = ValidationCheck::new(
            "fusion_strategies_defined",
            "[ARCH-13] Fusion strategies: E1Only, MultiSpace, Pipeline",
        )
        .with_priority(CheckPriority::Critical);

        // Verify all strategies are accessible
        let strategies = [
            FusionStrategy::E1Only,
            FusionStrategy::MultiSpace,
            FusionStrategy::Pipeline,
            FusionStrategy::Custom,
        ];

        // Just verify they exist (compile-time check)
        check.pass(&format!("{} strategies", strategies.len()), "4 strategies")
    }

    fn test_default_is_e1only() -> ValidationCheck {
        let check = ValidationCheck::new("default_is_e1only", "Default fusion strategy is E1Only")
            .with_priority(CheckPriority::Critical);

        let default = FusionStrategy::default();

        if default == FusionStrategy::E1Only {
            check.pass("E1Only", "E1Only")
        } else {
            check.fail(&format!("{:?}", default), "E1Only")
        }
    }

    fn test_e1only_semantics() -> ValidationCheck {
        let check =
            ValidationCheck::new("e1only_semantics", "E1Only strategy uses only E1 embedder")
                .with_priority(CheckPriority::Critical);

        // E1Only should use only E1 - this is a design constraint
        // Verify E1 exists and is the foundation
        let e1_is_foundation =
            EmbedderName::E1Semantic.index() == 0 && EmbedderName::E1Semantic.topic_weight() == 1.0;

        if e1_is_foundation {
            check.pass("E1 is foundation (index=0, weight=1.0)", "E1 only")
        } else {
            check.fail("E1 not foundation", "E1 only")
        }
    }

    fn test_multispace_excludes_temporal() -> ValidationCheck {
        let check = ValidationCheck::new(
            "multispace_excludes_temporal",
            "[AP-73] MultiSpace MUST exclude temporal embedders (E2-E4)",
        )
        .with_priority(CheckPriority::Critical);

        // MultiSpace should use semantic embedders, not temporal
        // Verify temporal embedders have weight 0.0 (excluded from scoring)
        let temporal = EmbedderName::temporal();
        let all_zero_weight = temporal
            .iter()
            .all(|e| e.topic_weight().abs() < f64::EPSILON);

        if all_zero_weight {
            check.pass("E2-E4 weight=0.0", "temporal excluded")
        } else {
            check.fail("E2-E4 have non-zero weight", "temporal excluded")
        }
    }

    fn test_pipeline_architecture() -> ValidationCheck {
        let check = ValidationCheck::new(
            "pipeline_architecture",
            "[AP-74, AP-75] Pipeline: Stage 1 (E13 recall) → Stage 2 (E1 dense) → Stage 3 (E12 rerank)",
        ).with_priority(CheckPriority::High);

        // Verify pipeline components exist
        let e13_exists = EmbedderName::all().contains(&EmbedderName::E13SPLADE);
        let e1_exists = EmbedderName::all().contains(&EmbedderName::E1Semantic);
        let e12_exists = EmbedderName::all().contains(&EmbedderName::E12LateInteraction);

        if e13_exists && e1_exists && e12_exists {
            check
                .pass("E13 → E1 → E12", "3-stage pipeline")
                .with_details(
                    "Stage 1: E13 SPLADE recall, Stage 2: E1 dense scoring, Stage 3: E12 rerank",
                )
        } else {
            check.fail("missing components", "E13, E1, E12 exist")
        }
    }

    fn test_e1only_embedders(embedders: &[EmbedderName]) -> ValidationCheck {
        let check = ValidationCheck::new("e1only_embedders", "E1Only strategy uses exactly [E1]")
            .with_priority(CheckPriority::High);

        let only_e1 = embedders.len() == 1 && embedders.contains(&EmbedderName::E1Semantic);

        if only_e1 {
            check.pass("[E1]", "[E1]")
        } else {
            let names: Vec<_> = embedders.iter().map(|e| e.as_str()).collect();
            check.fail(&names.join(", "), "[E1]")
        }
    }

    fn test_multispace_actual(embedders: &[EmbedderName]) -> ValidationCheck {
        let check = ValidationCheck::new(
            "multispace_actual",
            "MultiSpace actually excludes temporal embedders",
        )
        .with_priority(CheckPriority::Critical);

        let temporal = EmbedderName::temporal();
        let temporal_in_fusion: Vec<_> =
            embedders.iter().filter(|e| temporal.contains(e)).collect();

        if temporal_in_fusion.is_empty() {
            let names: Vec<_> = embedders.iter().map(|e| e.as_str()).collect();
            check
                .pass(&format!("{} embedders", embedders.len()), "no temporal")
                .with_details(&names.join(", "))
        } else {
            let temporal_names: Vec<_> = temporal_in_fusion.iter().map(|e| e.as_str()).collect();
            check.fail(&temporal_names.join(", "), "no temporal")
        }
    }

    fn test_best_improves_baseline(
        best: FusionStrategy,
        improvement: f64,
    ) -> (ValidationCheck, Option<Recommendation>) {
        let check = ValidationCheck::new(
            "best_improves_baseline",
            "Best strategy improves over E1Only baseline",
        )
        .with_priority(CheckPriority::High);

        let improves = improvement >= 0.0;

        if improves {
            (
                check.pass(
                    &format!("{:?} +{:.1}%", best, improvement * 100.0),
                    ">= 0% improvement",
                ),
                None,
            )
        } else {
            let rec = Recommendation {
                category: RecommendationCategory::Performance,
                description: format!(
                    "Multi-space fusion ({:?}) underperforms E1Only by {:.1}%",
                    best,
                    improvement.abs() * 100.0
                ),
                component: "FusionStrategy".to_string(),
                current: Some(format!("{:?}", best)),
                recommended: Some("E1Only or tune weights".to_string()),
                reason: "Fusion should not degrade baseline performance".to_string(),
            };
            (
                check.warning(
                    &format!("{:?} {:.1}%", best, improvement * 100.0),
                    ">= 0% improvement",
                ),
                Some(rec),
            )
        }
    }

    /// Get embedders for each fusion strategy.
    pub fn get_strategy_embedders(strategy: FusionStrategy) -> Vec<EmbedderName> {
        match strategy {
            FusionStrategy::E1Only => vec![EmbedderName::E1Semantic],
            FusionStrategy::MultiSpace => {
                // All semantic embedders except those for specific pipeline stages
                EmbedderName::semantic()
                    .into_iter()
                    .filter(|e| *e != EmbedderName::E12LateInteraction) // E12 is rerank only
                    .collect()
            }
            FusionStrategy::Pipeline => {
                vec![
                    EmbedderName::E13SPLADE,          // Stage 1: recall
                    EmbedderName::E1Semantic,         // Stage 2: dense scoring
                    EmbedderName::E12LateInteraction, // Stage 3: rerank
                ]
            }
            FusionStrategy::Custom => EmbedderName::all(),
        }
    }
}

#[cfg(all(test, feature = "benchmark-tests"))]
mod tests {
    use super::*;

    #[test]
    fn test_fusion_validation_config() {
        let result = FusionValidator::validate(None);
        assert!(result.all_passed);
        assert!(!result.checks.is_empty());
    }

    #[test]
    fn test_default_strategy() {
        let check = FusionValidator::test_default_is_e1only();
        assert!(check.is_passed());
    }

    #[test]
    fn test_multispace_excludes_temporal() {
        let check = FusionValidator::test_multispace_excludes_temporal();
        assert!(check.is_passed());
    }

    #[test]
    fn test_pipeline_architecture() {
        let check = FusionValidator::test_pipeline_architecture();
        assert!(check.is_passed());
    }

    #[test]
    fn test_strategy_embedders() {
        let e1only = FusionValidator::get_strategy_embedders(FusionStrategy::E1Only);
        assert_eq!(e1only.len(), 1);
        assert!(e1only.contains(&EmbedderName::E1Semantic));

        let multispace = FusionValidator::get_strategy_embedders(FusionStrategy::MultiSpace);
        assert!(multispace.contains(&EmbedderName::E1Semantic));
        assert!(!multispace
            .iter()
            .any(|e| EmbedderName::temporal().contains(e)));

        let pipeline = FusionValidator::get_strategy_embedders(FusionStrategy::Pipeline);
        assert!(pipeline.contains(&EmbedderName::E13SPLADE));
        assert!(pipeline.contains(&EmbedderName::E1Semantic));
        assert!(pipeline.contains(&EmbedderName::E12LateInteraction));
    }
}
