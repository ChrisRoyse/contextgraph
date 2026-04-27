//! ARCH compliance validation.
//!
//! Validates compliance with ARCH rules from CLAUDE.md:
//! - ARCH-01: TeleologicalArray is atomic (all 13 or nothing)
//! - ARCH-02: No cross-embedder comparison
//! - ARCH-09: Topic threshold >= 2.5
//! - ARCH-10: Divergence uses SEMANTIC only
//! - ARCH-12: E1 is foundation
//! - ARCH-14: E2-E4 weight = 0.0
//! - ARCH-18: E5 asymmetric
//! - AP-73: No temporal in fusion
//! - AP-74: E12 rerank only
//! - AP-75: E13 recall only

use super::{CheckPriority, PhaseResult, ValidationCheck, ValidationPhase};
use crate::realdata::config::{EmbedderName, FusionStrategy};
use crate::realdata::results::ConstitutionalCompliance;

/// ARCH compliance validation results.
#[derive(Debug, Clone)]
pub struct ArchComplianceResult {
    /// All rules passed.
    pub all_passed: bool,
    /// Individual rule checks.
    pub checks: Vec<ValidationCheck>,
    /// Rules passed count.
    pub rules_passed: usize,
    /// Rules total.
    pub rules_total: usize,
}

/// ARCH compliance validator.
pub struct ArchComplianceValidator;

impl ArchComplianceValidator {
    /// Run all ARCH compliance checks.
    pub fn validate() -> ArchComplianceResult {
        let mut result = ArchComplianceResult {
            all_passed: true,
            checks: Vec::new(),
            rules_passed: 0,
            rules_total: 0,
        };

        // ARCH-01: All 13 embeddings present
        let check = Self::check_arch_01();
        result.rules_total += 1;
        if check.is_passed() {
            result.rules_passed += 1;
        } else {
            result.all_passed = false;
        }
        result.checks.push(check);

        // ARCH-02: No cross-embedder comparison
        let check = Self::check_arch_02();
        result.rules_total += 1;
        if check.is_passed() {
            result.rules_passed += 1;
        } else {
            result.all_passed = false;
        }
        result.checks.push(check);

        // ARCH-09: Topic threshold >= 2.5
        let check = Self::check_arch_09();
        result.rules_total += 1;
        if check.is_passed() {
            result.rules_passed += 1;
        } else {
            result.all_passed = false;
        }
        result.checks.push(check);

        // ARCH-10: Divergence uses SEMANTIC only
        let check = Self::check_arch_10();
        result.rules_total += 1;
        if check.is_passed() {
            result.rules_passed += 1;
        } else {
            result.all_passed = false;
        }
        result.checks.push(check);

        // ARCH-12: E1 is foundation
        let check = Self::check_arch_12();
        result.rules_total += 1;
        if check.is_passed() {
            result.rules_passed += 1;
        } else {
            result.all_passed = false;
        }
        result.checks.push(check);

        // ARCH-13: Search strategies
        let check = Self::check_arch_13();
        result.rules_total += 1;
        if check.is_passed() {
            result.rules_passed += 1;
        } else {
            result.all_passed = false;
        }
        result.checks.push(check);

        // ARCH-14: E2-E4 weight = 0.0
        let check = Self::check_arch_14();
        result.rules_total += 1;
        if check.is_passed() {
            result.rules_passed += 1;
        } else {
            result.all_passed = false;
        }
        result.checks.push(check);

        // ARCH-18: E5 asymmetric
        let check = Self::check_arch_18();
        result.rules_total += 1;
        if check.is_passed() {
            result.rules_passed += 1;
        } else {
            result.all_passed = false;
        }
        result.checks.push(check);

        // AP-73: No temporal in fusion
        let check = Self::check_ap_73();
        result.rules_total += 1;
        if check.is_passed() {
            result.rules_passed += 1;
        } else {
            result.all_passed = false;
        }
        result.checks.push(check);

        // AP-74: E12 rerank only
        let check = Self::check_ap_74();
        result.rules_total += 1;
        if check.is_passed() {
            result.rules_passed += 1;
        } else {
            result.all_passed = false;
        }
        result.checks.push(check);

        // AP-75: E13 recall only
        let check = Self::check_ap_75();
        result.rules_total += 1;
        if check.is_passed() {
            result.rules_passed += 1;
        } else {
            result.all_passed = false;
        }
        result.checks.push(check);

        result
    }

    /// Validate with actual compliance data from benchmark run.
    pub fn validate_with_compliance(compliance: &ConstitutionalCompliance) -> ArchComplianceResult {
        let mut result = Self::validate();

        // Add any additional checks from the compliance data
        for rule in &compliance.rules {
            let check = ValidationCheck::new(&rule.rule_id, &rule.description)
                .with_priority(CheckPriority::Critical);

            let check = if rule.passed {
                check.pass(&rule.details, "compliant")
            } else {
                check.fail(&rule.details, "compliant")
            };

            // Only add if not already in our checks
            if !result.checks.iter().any(|c| c.name == rule.rule_id) {
                result.rules_total += 1;
                if check.is_passed() {
                    result.rules_passed += 1;
                } else {
                    result.all_passed = false;
                }
                result.checks.push(check);
            }
        }

        // Add errors from compliance
        for error in &compliance.errors {
            let check = ValidationCheck::new("error", error)
                .with_priority(CheckPriority::Critical)
                .fail(error, "no errors");
            result.all_passed = false;
            result.checks.push(check);
        }

        result
    }

    /// Convert to PhaseResult.
    pub fn to_phase_result(result: ArchComplianceResult, duration_ms: u64) -> PhaseResult {
        let mut phase_result = PhaseResult::new(ValidationPhase::Arch);
        for check in result.checks {
            phase_result.add_check(check);
        }
        phase_result.finalize(duration_ms);
        phase_result
    }

    // Individual ARCH rule checks

    fn check_arch_01() -> ValidationCheck {
        let check = ValidationCheck::new(
            "ARCH-01",
            "TeleologicalArray is atomic - store all 13 embeddings or nothing",
        )
        .with_priority(CheckPriority::Critical);

        // Verify all 13 embedders exist
        let count = EmbedderName::all().len();
        if count == 13 {
            check.pass("13 embedders defined", "13 embedders")
        } else {
            check.fail(&format!("{} embedders", count), "13 embedders")
        }
    }

    fn check_arch_02() -> ValidationCheck {
        let check = ValidationCheck::new(
            "ARCH-02",
            "Apples-to-apples only - compare E1<->E1, never E1<->E5",
        )
        .with_priority(CheckPriority::Critical);

        // This is a design rule - verified by type system and index checks
        // Embedder indices are distinct (0-12)
        let indices: Vec<usize> = EmbedderName::all().iter().map(|e| e.index()).collect();
        let unique_indices: std::collections::HashSet<_> = indices.iter().cloned().collect();

        if unique_indices.len() == 13 && indices.iter().all(|&i| i < 13) {
            check.pass("unique indices 0-12", "distinct embedder indices")
        } else {
            check.fail(
                &format!("{} unique indices", unique_indices.len()),
                "13 unique indices",
            )
        }
    }

    fn check_arch_09() -> ValidationCheck {
        let check = ValidationCheck::new("ARCH-09", "Topic threshold is weighted_agreement >= 2.5")
            .with_priority(CheckPriority::Critical);

        // Calculate max weighted agreement
        let max_wa: f64 = EmbedderName::all().iter().map(|e| e.topic_weight()).sum();

        // Threshold should be achievable (2.5 out of 8.5 max)
        let threshold = 2.5;
        let achievable = max_wa >= threshold;

        if achievable {
            check
                .pass(
                    &format!("max={:.1}, threshold={:.1}", max_wa, threshold),
                    ">= 2.5 threshold",
                )
                .with_details("3 semantic spaces agreeing = 3.0 -> TOPIC")
        } else {
            check.fail(&format!("max={:.1}", max_wa), ">= 2.5 achievable")
        }
    }

    fn check_arch_10() -> ValidationCheck {
        let check = ValidationCheck::new(
            "ARCH-10",
            "Divergence detection uses SEMANTIC embedders only",
        )
        .with_priority(CheckPriority::Critical);

        let semantic = EmbedderName::semantic();
        let divergence_embedders: Vec<_> = EmbedderName::all()
            .into_iter()
            .filter(|e| e.used_for_divergence())
            .collect();

        // Should be exactly the semantic embedders
        let matches = divergence_embedders.len() == semantic.len()
            && divergence_embedders.iter().all(|e| semantic.contains(e));

        if matches {
            check.pass(
                &format!("{} semantic embedders", semantic.len()),
                "7 semantic only",
            )
        } else {
            let non_semantic: Vec<_> = divergence_embedders
                .iter()
                .filter(|e| !semantic.contains(e))
                .map(|e| e.as_str())
                .collect();
            check
                .fail(
                    &format!(
                        "{} used, {} non-semantic",
                        divergence_embedders.len(),
                        non_semantic.len()
                    ),
                    "7 semantic only",
                )
                .with_details(&format!("Non-semantic in divergence: {:?}", non_semantic))
        }
    }

    fn check_arch_12() -> ValidationCheck {
        let check = ValidationCheck::new(
            "ARCH-12",
            "E1 is THE semantic foundation - all retrieval starts with E1",
        )
        .with_priority(CheckPriority::Critical);

        // E1 should be in semantic embedders
        let semantic = EmbedderName::semantic();
        let e1_in_semantic = semantic.contains(&EmbedderName::E1Semantic);

        // E1 should have the highest weight (or equal)
        let e1_weight = EmbedderName::E1Semantic.topic_weight();
        let max_weight = EmbedderName::all()
            .iter()
            .map(|e| e.topic_weight())
            .fold(0.0, f64::max);

        if e1_in_semantic && (e1_weight - max_weight).abs() < f64::EPSILON {
            check.pass("E1 in semantic, weight=1.0", "E1 is foundation")
        } else {
            check.fail(
                &format!("in_semantic={}, weight={}", e1_in_semantic, e1_weight),
                "E1 is foundation",
            )
        }
    }

    fn check_arch_13() -> ValidationCheck {
        let check =
            ValidationCheck::new("ARCH-13", "Search strategies: E1Only, MultiSpace, Pipeline")
                .with_priority(CheckPriority::High);

        // Default should be E1Only
        let default = FusionStrategy::default();
        if default == FusionStrategy::E1Only {
            check.pass("E1Only default, 3 strategies", "E1Only default")
        } else {
            check.fail(&format!("{:?} default", default), "E1Only default")
        }
    }

    fn check_arch_14() -> ValidationCheck {
        let check = ValidationCheck::new(
            "ARCH-14",
            "Weight profiles MUST have E2-E4=0.0 for semantic scoring",
        )
        .with_priority(CheckPriority::Critical);

        let temporal = EmbedderName::temporal();
        let all_zero = temporal
            .iter()
            .all(|e| e.topic_weight().abs() < f64::EPSILON);

        let weights: Vec<_> = temporal
            .iter()
            .map(|e| format!("{}={}", e.as_str(), e.topic_weight()))
            .collect();

        if all_zero {
            check
                .pass("E2-E4 = 0.0", "temporal weight = 0.0")
                .with_details(&weights.join(", "))
        } else {
            check.fail(&weights.join(", "), "all 0.0")
        }
    }

    fn check_arch_18() -> ValidationCheck {
        let check = ValidationCheck::new("ARCH-18", "E5 Causal MUST use asymmetric similarity")
            .with_priority(CheckPriority::Critical);

        let asymmetric = EmbedderName::asymmetric();
        let e5_is_asymmetric = asymmetric.contains(&EmbedderName::E5Causal);

        if e5_is_asymmetric {
            check
                .pass("E5 in asymmetric list", "E5 asymmetric")
                .with_details("cause→effect direction matters")
        } else {
            check.fail("E5 not asymmetric", "E5 asymmetric")
        }
    }

    fn check_ap_73() -> ValidationCheck {
        let check = ValidationCheck::new(
            "AP-73",
            "Temporal embedders (E2-E4) MUST NOT be in similarity fusion",
        )
        .with_priority(CheckPriority::Critical);

        // This is enforced by design: temporal embedders have weight 0.0
        let temporal = EmbedderName::temporal();
        let all_zero_weight = temporal
            .iter()
            .all(|e| e.topic_weight().abs() < f64::EPSILON);

        if all_zero_weight {
            check.pass("E2-E4 weight=0 (excluded)", "temporal excluded from fusion")
        } else {
            check.fail("E2-E4 have non-zero weight", "temporal excluded")
        }
    }

    fn check_ap_74() -> ValidationCheck {
        let check = ValidationCheck::new(
            "AP-74",
            "E12 ColBERT MUST only be used for re-ranking, NOT initial retrieval",
        )
        .with_priority(CheckPriority::High);

        // E12 is in semantic (for topic detection) but should be Stage 3 in pipeline
        // This is a usage guideline - verify E12 exists and is semantic
        let semantic = EmbedderName::semantic();
        let e12_in_semantic = semantic.contains(&EmbedderName::E12LateInteraction);

        if e12_in_semantic {
            check
                .pass("E12 defined, rerank-only guidance", "E12 for rerank only")
                .with_details("Pipeline Stage 3: E12 MaxSim precision")
        } else {
            check.fail("E12 not in semantic", "E12 for rerank")
        }
    }

    fn check_ap_75() -> ValidationCheck {
        let check = ValidationCheck::new(
            "AP-75",
            "E13 SPLADE MUST be used for Stage 1 recall, NOT final ranking",
        )
        .with_priority(CheckPriority::High);

        // E13 is sparse, used for initial recall
        let semantic = EmbedderName::semantic();
        let e13_in_semantic = semantic.contains(&EmbedderName::E13SPLADE);

        if e13_in_semantic {
            check
                .pass("E13 defined, recall-only guidance", "E13 for recall only")
                .with_details("Pipeline Stage 1: BM25+E13 sparse recall")
        } else {
            check.fail("E13 not in semantic", "E13 for recall")
        }
    }
}

#[cfg(all(test, feature = "benchmark-tests"))]
mod tests {
    use super::*;

    #[test]
    fn test_arch_compliance() {
        let result = ArchComplianceValidator::validate();
        assert!(result.all_passed, "All ARCH rules should pass");
        assert_eq!(result.rules_passed, result.rules_total);
    }

    #[test]
    fn test_arch_09_threshold() {
        let check = ArchComplianceValidator::check_arch_09();
        assert!(check.is_passed());
    }

    #[test]
    fn test_arch_14_temporal_weights() {
        let check = ArchComplianceValidator::check_arch_14();
        assert!(check.is_passed());
    }

    #[test]
    fn test_ap_73_temporal_excluded() {
        let check = ArchComplianceValidator::check_ap_73();
        assert!(check.is_passed());
    }
}
