//! Config validation for EmbedderName enum and configuration.
//!
//! Tests:
//! - All 13 embedders exist
//! - Category assignments (semantic, temporal, relational, structural)
//! - Topic weights per ARCH-09
//! - Max weighted_agreement = 8.5

use super::{CheckPriority, PhaseResult, ValidationCheck, ValidationPhase};
use crate::realdata::config::{EmbedderName, FusionStrategy};

/// Config validation results.
#[derive(Debug, Clone)]
pub struct ConfigValidationResult {
    /// All checks passed.
    pub all_passed: bool,
    /// Individual check results.
    pub checks: Vec<ValidationCheck>,
    /// Embedder count.
    pub embedder_count: usize,
    /// Max weighted agreement.
    pub max_weighted_agreement: f64,
    /// Category counts.
    pub category_counts: CategoryCounts,
}

/// Category counts for embedders.
#[derive(Debug, Clone, Default)]
pub struct CategoryCounts {
    pub semantic: usize,
    pub temporal: usize,
    pub relational: usize,
    pub structural: usize,
    pub asymmetric: usize,
}

/// Config validator.
pub struct ConfigValidator;

impl ConfigValidator {
    /// Run all config validation checks.
    pub fn validate() -> ConfigValidationResult {
        let mut result = ConfigValidationResult {
            all_passed: true,
            checks: Vec::new(),
            embedder_count: 0,
            max_weighted_agreement: 0.0,
            category_counts: CategoryCounts::default(),
        };

        // Test 1: All 13 embedders exist
        let check = Self::test_all_embedders_count();
        if !check.is_passed() {
            result.all_passed = false;
        }
        result.embedder_count = EmbedderName::all().len();
        result.checks.push(check);

        // Test 2: Semantic embedders (7)
        let check = Self::test_semantic_embedders();
        if !check.is_passed() {
            result.all_passed = false;
        }
        result.category_counts.semantic = EmbedderName::semantic().len();
        result.checks.push(check);

        // Test 3: Temporal embedders (3)
        let check = Self::test_temporal_embedders();
        if !check.is_passed() {
            result.all_passed = false;
        }
        result.category_counts.temporal = EmbedderName::temporal().len();
        result.checks.push(check);

        // Test 4: Relational embedders (2)
        let check = Self::test_relational_embedders();
        if !check.is_passed() {
            result.all_passed = false;
        }
        result.category_counts.relational = EmbedderName::relational().len();
        result.checks.push(check);

        // Test 5: Structural embedders (1)
        let check = Self::test_structural_embedders();
        if !check.is_passed() {
            result.all_passed = false;
        }
        result.category_counts.structural = EmbedderName::structural().len();
        result.checks.push(check);

        // Test 6: Asymmetric embedders (3)
        let check = Self::test_asymmetric_embedders();
        if !check.is_passed() {
            result.all_passed = false;
        }
        result.category_counts.asymmetric = EmbedderName::asymmetric().len();
        result.checks.push(check);

        // Test 7: Category sum equals total
        let check = Self::test_category_sum();
        if !check.is_passed() {
            result.all_passed = false;
        }
        result.checks.push(check);

        // Test 8: Semantic weights = 1.0
        let check = Self::test_semantic_weights();
        if !check.is_passed() {
            result.all_passed = false;
        }
        result.checks.push(check);

        // Test 9: Temporal weights = 0.0 (CRITICAL - ARCH-14)
        let check = Self::test_temporal_weights();
        if !check.is_passed() {
            result.all_passed = false;
        }
        result.checks.push(check);

        // Test 10: Relational/structural weights = 0.5
        let check = Self::test_relational_structural_weights();
        if !check.is_passed() {
            result.all_passed = false;
        }
        result.checks.push(check);

        // Test 11: Max weighted_agreement = 8.5
        let check = Self::test_max_weighted_agreement();
        result.max_weighted_agreement = Self::compute_max_weighted_agreement();
        if !check.is_passed() {
            result.all_passed = false;
        }
        result.checks.push(check);

        // Test 12: E1 is foundation
        let check = Self::test_e1_is_foundation();
        if !check.is_passed() {
            result.all_passed = false;
        }
        result.checks.push(check);

        // Test 13: Default fusion strategy is E1Only
        let check = Self::test_default_fusion_strategy();
        if !check.is_passed() {
            result.all_passed = false;
        }
        result.checks.push(check);

        // Test 14: Embedder indices are unique 0-12
        let check = Self::test_embedder_indices();
        if !check.is_passed() {
            result.all_passed = false;
        }
        result.checks.push(check);

        // Test 15: Divergence embedders are semantic only
        let check = Self::test_divergence_embedders();
        if !check.is_passed() {
            result.all_passed = false;
        }
        result.checks.push(check);

        result
    }

    /// Convert to PhaseResult.
    pub fn to_phase_result(result: ConfigValidationResult, duration_ms: u64) -> PhaseResult {
        let mut phase_result = PhaseResult::new(ValidationPhase::Config);
        for check in result.checks {
            phase_result.add_check(check);
        }
        phase_result.finalize(duration_ms);
        phase_result
    }

    // Individual test functions

    fn test_all_embedders_count() -> ValidationCheck {
        let count = EmbedderName::all().len();
        let check = ValidationCheck::new("all_embedders_count", "All 13 embedders exist")
            .with_priority(CheckPriority::Critical);

        if count == 13 {
            check.pass(&count.to_string(), "13")
        } else {
            check.fail(&count.to_string(), "13")
        }
    }

    fn test_semantic_embedders() -> ValidationCheck {
        let semantic = EmbedderName::semantic();
        let count = semantic.len();
        let check = ValidationCheck::new(
            "semantic_embedders_count",
            "Semantic embedders = 7 (E1, E5, E6, E7, E10, E12, E13)",
        )
        .with_priority(CheckPriority::Critical);

        // Verify correct embedders
        let expected = [
            EmbedderName::E1Semantic,
            EmbedderName::E5Causal,
            EmbedderName::E6Sparse,
            EmbedderName::E7Code,
            EmbedderName::E10Multimodal,
            EmbedderName::E12LateInteraction,
            EmbedderName::E13SPLADE,
        ];

        let all_correct = expected.iter().all(|e| semantic.contains(e));

        if count == 7 && all_correct {
            check
                .pass(&count.to_string(), "7")
                .with_details("E1, E5, E6, E7, E10, E12, E13")
        } else {
            check.fail(&count.to_string(), "7")
        }
    }

    fn test_temporal_embedders() -> ValidationCheck {
        let temporal = EmbedderName::temporal();
        let count = temporal.len();
        let check = ValidationCheck::new(
            "temporal_embedders_count",
            "Temporal embedders = 3 (E2, E3, E4)",
        )
        .with_priority(CheckPriority::Critical);

        let expected = [
            EmbedderName::E2Recency,
            EmbedderName::E3Periodic,
            EmbedderName::E4Sequence,
        ];

        let all_correct = expected.iter().all(|e| temporal.contains(e));

        if count == 3 && all_correct {
            check
                .pass(&count.to_string(), "3")
                .with_details("E2, E3, E4")
        } else {
            check.fail(&count.to_string(), "3")
        }
    }

    fn test_relational_embedders() -> ValidationCheck {
        let relational = EmbedderName::relational();
        let count = relational.len();
        let check = ValidationCheck::new(
            "relational_embedders_count",
            "Relational embedders = 2 (E8, E11)",
        )
        .with_priority(CheckPriority::High);

        let expected = [EmbedderName::E8Graph, EmbedderName::E11Entity];
        let all_correct = expected.iter().all(|e| relational.contains(e));

        if count == 2 && all_correct {
            check.pass(&count.to_string(), "2").with_details("E8, E11")
        } else {
            check.fail(&count.to_string(), "2")
        }
    }

    fn test_structural_embedders() -> ValidationCheck {
        let structural = EmbedderName::structural();
        let count = structural.len();
        let check = ValidationCheck::new(
            "structural_embedders_count",
            "Structural embedders = 1 (E9)",
        )
        .with_priority(CheckPriority::High);

        let expected = [EmbedderName::E9HDC];
        let all_correct = expected.iter().all(|e| structural.contains(e));

        if count == 1 && all_correct {
            check.pass(&count.to_string(), "1").with_details("E9")
        } else {
            check.fail(&count.to_string(), "1")
        }
    }

    fn test_asymmetric_embedders() -> ValidationCheck {
        let asymmetric = EmbedderName::asymmetric();
        let count = asymmetric.len();
        let check = ValidationCheck::new(
            "asymmetric_embedders_count",
            "Asymmetric embedders = 3 (E5, E8, E10)",
        )
        .with_priority(CheckPriority::High);

        let expected = [
            EmbedderName::E5Causal,
            EmbedderName::E8Graph,
            EmbedderName::E10Multimodal,
        ];

        let all_correct = expected.iter().all(|e| asymmetric.contains(e));

        if count == 3 && all_correct {
            check
                .pass(&count.to_string(), "3")
                .with_details("E5, E8, E10")
        } else {
            check.fail(&count.to_string(), "3")
        }
    }

    fn test_category_sum() -> ValidationCheck {
        let semantic = EmbedderName::semantic().len();
        let temporal = EmbedderName::temporal().len();
        let relational = EmbedderName::relational().len();
        let structural = EmbedderName::structural().len();

        let sum = semantic + temporal + relational + structural;
        let check = ValidationCheck::new("category_sum", "Category sum = 13 (7 + 3 + 2 + 1)")
            .with_priority(CheckPriority::Critical);

        if sum == 13 {
            check.pass(&sum.to_string(), "13").with_details(&format!(
                "{} + {} + {} + {} = {}",
                semantic, temporal, relational, structural, sum
            ))
        } else {
            check.fail(&sum.to_string(), "13")
        }
    }

    fn test_semantic_weights() -> ValidationCheck {
        let semantic = EmbedderName::semantic();
        let check = ValidationCheck::new(
            "semantic_weights",
            "All semantic embedders have topic_weight = 1.0",
        )
        .with_priority(CheckPriority::Critical);

        let all_correct = semantic
            .iter()
            .all(|e| (e.topic_weight() - 1.0).abs() < f64::EPSILON);
        let weights: Vec<_> = semantic
            .iter()
            .map(|e| format!("{}={}", e.as_str(), e.topic_weight()))
            .collect();

        if all_correct {
            check.pass("1.0", "1.0").with_details(&weights.join(", "))
        } else {
            check.fail(&weights.join(", "), "all 1.0")
        }
    }

    fn test_temporal_weights() -> ValidationCheck {
        let temporal = EmbedderName::temporal();
        let check = ValidationCheck::new(
            "temporal_weights",
            "[ARCH-14] Temporal embedders have topic_weight = 0.0",
        )
        .with_priority(CheckPriority::Critical);

        let all_correct = temporal
            .iter()
            .all(|e| e.topic_weight().abs() < f64::EPSILON);
        let weights: Vec<_> = temporal
            .iter()
            .map(|e| format!("{}={}", e.as_str(), e.topic_weight()))
            .collect();

        if all_correct {
            check.pass("0.0", "0.0").with_details(&weights.join(", "))
        } else {
            check.fail(&weights.join(", "), "all 0.0")
        }
    }

    fn test_relational_structural_weights() -> ValidationCheck {
        let relational = EmbedderName::relational();
        let structural = EmbedderName::structural();
        let check = ValidationCheck::new(
            "relational_structural_weights",
            "Relational and structural embedders have topic_weight = 0.5",
        )
        .with_priority(CheckPriority::Critical);

        let all_relational_correct = relational
            .iter()
            .all(|e| (e.topic_weight() - 0.5).abs() < f64::EPSILON);
        let all_structural_correct = structural
            .iter()
            .all(|e| (e.topic_weight() - 0.5).abs() < f64::EPSILON);

        let weights: Vec<_> = relational
            .iter()
            .chain(structural.iter())
            .map(|e| format!("{}={}", e.as_str(), e.topic_weight()))
            .collect();

        if all_relational_correct && all_structural_correct {
            check.pass("0.5", "0.5").with_details(&weights.join(", "))
        } else {
            check.fail(&weights.join(", "), "all 0.5")
        }
    }

    fn compute_max_weighted_agreement() -> f64 {
        let semantic_contribution: f64 = EmbedderName::semantic()
            .iter()
            .map(|e| e.topic_weight())
            .sum();
        let relational_contribution: f64 = EmbedderName::relational()
            .iter()
            .map(|e| e.topic_weight())
            .sum();
        let structural_contribution: f64 = EmbedderName::structural()
            .iter()
            .map(|e| e.topic_weight())
            .sum();
        // Temporal = 0.0

        semantic_contribution + relational_contribution + structural_contribution
    }

    fn test_max_weighted_agreement() -> ValidationCheck {
        let max_wa = Self::compute_max_weighted_agreement();
        let expected = 8.5; // 7*1.0 + 2*0.5 + 1*0.5 = 8.5
        let check = ValidationCheck::new(
            "max_weighted_agreement",
            "[ARCH-09] Max weighted_agreement = 8.5",
        )
        .with_priority(CheckPriority::Critical);

        if (max_wa - expected).abs() < f64::EPSILON {
            check
                .pass(&format!("{:.1}", max_wa), "8.5")
                .with_details("7*1.0 (semantic) + 2*0.5 (relational) + 1*0.5 (structural) = 8.5")
        } else {
            check.fail(&format!("{:.2}", max_wa), "8.5")
        }
    }

    fn test_e1_is_foundation() -> ValidationCheck {
        let semantic = EmbedderName::semantic();
        let check = ValidationCheck::new(
            "e1_is_foundation",
            "[ARCH-12] E1 is in semantic embedders (foundation)",
        )
        .with_priority(CheckPriority::Critical);

        let e1_in_semantic = semantic.contains(&EmbedderName::E1Semantic);

        if e1_in_semantic {
            check.pass("E1 in semantic", "E1 in semantic")
        } else {
            check.fail("E1 not in semantic", "E1 in semantic")
        }
    }

    fn test_default_fusion_strategy() -> ValidationCheck {
        let default = FusionStrategy::default();
        let check = ValidationCheck::new(
            "default_fusion_strategy",
            "[ARCH-13] Default fusion strategy is E1Only",
        )
        .with_priority(CheckPriority::High);

        if default == FusionStrategy::E1Only {
            check.pass("E1Only", "E1Only")
        } else {
            check.fail(&format!("{:?}", default), "E1Only")
        }
    }

    fn test_embedder_indices() -> ValidationCheck {
        let all = EmbedderName::all();
        let check = ValidationCheck::new("embedder_indices", "Embedder indices are unique 0-12")
            .with_priority(CheckPriority::High);

        let mut indices: Vec<usize> = all.iter().map(|e| e.index()).collect();
        indices.sort();

        let expected: Vec<usize> = (0..13).collect();

        if indices == expected {
            check.pass("0-12", "0-12")
        } else {
            check.fail(&format!("{:?}", indices), "0-12")
        }
    }

    fn test_divergence_embedders() -> ValidationCheck {
        let check = ValidationCheck::new(
            "divergence_embedders",
            "[ARCH-10] Divergence detection uses semantic embedders only",
        )
        .with_priority(CheckPriority::Critical);

        let semantic = EmbedderName::semantic();
        let divergence_embedders: Vec<_> = EmbedderName::all()
            .into_iter()
            .filter(|e| e.used_for_divergence())
            .collect();

        // Divergence embedders should be exactly the semantic embedders
        let all_match = divergence_embedders.len() == semantic.len()
            && divergence_embedders.iter().all(|e| semantic.contains(e));

        if all_match {
            check.pass(
                &format!("{} embedders", divergence_embedders.len()),
                "7 semantic",
            )
        } else {
            check
                .fail(
                    &format!("{} embedders", divergence_embedders.len()),
                    "7 semantic",
                )
                .with_details(&format!(
                    "Got: {:?}",
                    divergence_embedders
                        .iter()
                        .map(|e| e.as_str())
                        .collect::<Vec<_>>()
                ))
        }
    }
}

#[cfg(all(test, feature = "benchmark-tests"))]
mod tests {
    use super::*;

    #[test]
    fn test_config_validation() {
        let result = ConfigValidator::validate();
        assert!(result.all_passed, "Config validation should pass");
        assert_eq!(result.embedder_count, 13);
        assert!((result.max_weighted_agreement - 8.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_category_counts() {
        let result = ConfigValidator::validate();
        assert_eq!(result.category_counts.semantic, 7);
        assert_eq!(result.category_counts.temporal, 3);
        assert_eq!(result.category_counts.relational, 2);
        assert_eq!(result.category_counts.structural, 1);
        assert_eq!(result.category_counts.asymmetric, 3);
    }
}
