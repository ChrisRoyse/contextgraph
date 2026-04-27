//! Asymmetric embedder validation for E5/E8/E10.
//!
//! Tests:
//! - E5, E8, E10 asymmetric ratio (1.5 ± 0.15)
//! - Forward/reverse vector distinctness > 80%
//! - Direction modifier compliance

use std::collections::HashMap;

use super::{
    CheckPriority, PhaseResult, Recommendation, RecommendationCategory, ValidationCheck,
    ValidationPhase,
};
use crate::realdata::config::EmbedderName;

/// Asymmetric validation results.
#[derive(Debug, Clone)]
pub struct AsymmetricValidationResult {
    /// All checks passed.
    pub all_passed: bool,
    /// Individual check results.
    pub checks: Vec<ValidationCheck>,
    /// Per-embedder metrics.
    pub embedder_metrics: HashMap<EmbedderName, AsymmetricMetrics>,
    /// Recommendations.
    pub recommendations: Vec<Recommendation>,
}

/// Metrics for a single asymmetric embedder.
#[derive(Debug, Clone, Default)]
pub struct AsymmetricMetrics {
    /// Forward-to-reverse similarity ratio.
    pub asymmetric_ratio: f64,
    /// Percentage of pairs with distinctness (sim < 0.95).
    pub distinctness_percentage: f64,
    /// Number of pairs tested.
    pub num_pairs_tested: usize,
    /// Direction modifier used.
    pub direction_modifier: f64,
}

/// Expected asymmetric ratio and tolerance.
pub const EXPECTED_ASYMMETRIC_RATIO: f64 = 1.5;
pub const ASYMMETRIC_RATIO_TOLERANCE: f64 = 0.15;
pub const MIN_DISTINCTNESS: f64 = 0.80; // 80% of pairs should differ

/// Direction modifiers per CLAUDE.md.
pub const DIRECTION_MODIFIER_CAUSE_EFFECT: f64 = 1.2;
pub const DIRECTION_MODIFIER_EFFECT_CAUSE: f64 = 0.8;
pub const DIRECTION_MODIFIER_SAME: f64 = 1.0;

/// Asymmetric validator.
pub struct AsymmetricValidator;

impl AsymmetricValidator {
    /// Run all asymmetric validation checks.
    ///
    /// If `measured_ratios` is provided, validates actual benchmark results.
    /// Otherwise, validates the design/configuration.
    pub fn validate(
        measured_ratios: Option<&HashMap<EmbedderName, f64>>,
        measured_distinctness: Option<&HashMap<EmbedderName, f64>>,
    ) -> AsymmetricValidationResult {
        let mut result = AsymmetricValidationResult {
            all_passed: true,
            checks: Vec::new(),
            embedder_metrics: HashMap::new(),
            recommendations: Vec::new(),
        };

        // Test 1: Asymmetric embedders are defined
        let check = Self::test_asymmetric_embedders_defined();
        if !check.is_passed() {
            result.all_passed = false;
        }
        result.checks.push(check);

        // Test 2: E5 is in asymmetric list (ARCH-18)
        let check = Self::test_e5_is_asymmetric();
        if !check.is_passed() {
            result.all_passed = false;
        }
        result.checks.push(check);

        // Test 3: E8 is in asymmetric list
        let check = Self::test_e8_is_asymmetric();
        if !check.is_passed() {
            result.all_passed = false;
        }
        result.checks.push(check);

        // Test 4: E10 is in asymmetric list
        let check = Self::test_e10_is_asymmetric();
        if !check.is_passed() {
            result.all_passed = false;
        }
        result.checks.push(check);

        // Test 5: Direction modifier formula
        let check = Self::test_direction_modifiers();
        if !check.is_passed() {
            result.all_passed = false;
        }
        result.checks.push(check);

        // If we have measured ratios, validate them
        if let Some(ratios) = measured_ratios {
            for (embedder, &ratio) in ratios {
                if EmbedderName::asymmetric().contains(embedder) {
                    let (check, _metrics, recommendation) =
                        Self::test_asymmetric_ratio(*embedder, ratio);

                    result.embedder_metrics.insert(
                        *embedder,
                        AsymmetricMetrics {
                            asymmetric_ratio: ratio,
                            ..Default::default()
                        },
                    );

                    if !check.is_passed() {
                        result.all_passed = false;
                    }
                    result.checks.push(check);

                    if let Some(rec) = recommendation {
                        result.recommendations.push(rec);
                    }
                }
            }
        }

        // If we have measured distinctness, validate them
        if let Some(distinctness) = measured_distinctness {
            for (embedder, &pct) in distinctness {
                if EmbedderName::asymmetric().contains(embedder) {
                    let (check, recommendation) = Self::test_vector_distinctness(*embedder, pct);

                    // Update metrics
                    if let Some(metrics) = result.embedder_metrics.get_mut(embedder) {
                        metrics.distinctness_percentage = pct;
                    } else {
                        result.embedder_metrics.insert(
                            *embedder,
                            AsymmetricMetrics {
                                distinctness_percentage: pct,
                                ..Default::default()
                            },
                        );
                    }

                    if !check.is_passed() {
                        result.all_passed = false;
                    }
                    result.checks.push(check);

                    if let Some(rec) = recommendation {
                        result.recommendations.push(rec);
                    }
                }
            }
        }

        result
    }

    /// Convert to PhaseResult.
    pub fn to_phase_result(result: AsymmetricValidationResult, duration_ms: u64) -> PhaseResult {
        let mut phase_result = PhaseResult::new(ValidationPhase::Asymmetric);
        for check in result.checks {
            phase_result.add_check(check);
        }
        phase_result.finalize(duration_ms);
        phase_result
    }

    // Individual tests

    fn test_asymmetric_embedders_defined() -> ValidationCheck {
        let check = ValidationCheck::new(
            "asymmetric_embedders_defined",
            "Asymmetric embedders = 3 (E5, E8, E10)",
        )
        .with_priority(CheckPriority::Critical);

        let asymmetric = EmbedderName::asymmetric();
        let count = asymmetric.len();

        if count == 3 {
            let names: Vec<_> = asymmetric.iter().map(|e| e.as_str()).collect();
            check
                .pass(&count.to_string(), "3")
                .with_details(&names.join(", "))
        } else {
            check.fail(&count.to_string(), "3")
        }
    }

    fn test_e5_is_asymmetric() -> ValidationCheck {
        let check = ValidationCheck::new(
            "e5_is_asymmetric",
            "[ARCH-18] E5 Causal uses asymmetric similarity",
        )
        .with_priority(CheckPriority::Critical);

        let asymmetric = EmbedderName::asymmetric();
        let e5_is_asymmetric = asymmetric.contains(&EmbedderName::E5Causal);

        if e5_is_asymmetric {
            check.pass("E5 in asymmetric", "E5 asymmetric")
        } else {
            check.fail("E5 not asymmetric", "E5 asymmetric")
        }
    }

    fn test_e8_is_asymmetric() -> ValidationCheck {
        let check = ValidationCheck::new("e8_is_asymmetric", "E8 Graph uses asymmetric similarity")
            .with_priority(CheckPriority::High);

        let asymmetric = EmbedderName::asymmetric();
        let e8_is_asymmetric = asymmetric.contains(&EmbedderName::E8Graph);

        if e8_is_asymmetric {
            check.pass("E8 in asymmetric", "E8 asymmetric")
        } else {
            check.fail("E8 not asymmetric", "E8 asymmetric")
        }
    }

    fn test_e10_is_asymmetric() -> ValidationCheck {
        let check = ValidationCheck::new(
            "e10_is_asymmetric",
            "E10 Multimodal uses asymmetric similarity",
        )
        .with_priority(CheckPriority::High);

        let asymmetric = EmbedderName::asymmetric();
        let e10_is_asymmetric = asymmetric.contains(&EmbedderName::E10Multimodal);

        if e10_is_asymmetric {
            check.pass("E10 in asymmetric", "E10 asymmetric")
        } else {
            check.fail("E10 not asymmetric", "E10 asymmetric")
        }
    }

    fn test_direction_modifiers() -> ValidationCheck {
        let check = ValidationCheck::new(
            "direction_modifiers",
            "Direction modifiers: cause→effect=1.2, effect→cause=0.8, same=1.0",
        )
        .with_priority(CheckPriority::High);

        // Verify the modifiers match expected values
        let valid = (DIRECTION_MODIFIER_CAUSE_EFFECT - 1.2).abs() < f64::EPSILON
            && (DIRECTION_MODIFIER_EFFECT_CAUSE - 0.8).abs() < f64::EPSILON
            && (DIRECTION_MODIFIER_SAME - 1.0).abs() < f64::EPSILON;

        if valid {
            check.pass("1.2, 0.8, 1.0", "1.2, 0.8, 1.0")
        } else {
            check.fail(
                &format!(
                    "{}, {}, {}",
                    DIRECTION_MODIFIER_CAUSE_EFFECT,
                    DIRECTION_MODIFIER_EFFECT_CAUSE,
                    DIRECTION_MODIFIER_SAME
                ),
                "1.2, 0.8, 1.0",
            )
        }
    }

    fn test_asymmetric_ratio(
        embedder: EmbedderName,
        ratio: f64,
    ) -> (ValidationCheck, AsymmetricMetrics, Option<Recommendation>) {
        let check = ValidationCheck::new(
            &format!("{}_asymmetric_ratio", embedder.as_str()),
            &format!(
                "{} asymmetric ratio within {} ± {}",
                embedder.as_str(),
                EXPECTED_ASYMMETRIC_RATIO,
                ASYMMETRIC_RATIO_TOLERANCE
            ),
        )
        .with_priority(CheckPriority::High);

        let metrics = AsymmetricMetrics {
            asymmetric_ratio: ratio,
            ..Default::default()
        };

        let lower = EXPECTED_ASYMMETRIC_RATIO - ASYMMETRIC_RATIO_TOLERANCE;
        let upper = EXPECTED_ASYMMETRIC_RATIO + ASYMMETRIC_RATIO_TOLERANCE;
        let in_range = ratio >= lower && ratio <= upper;

        let (check, recommendation) = if in_range {
            (
                check.pass(
                    &format!("{:.3}", ratio),
                    &format!("[{:.2}, {:.2}]", lower, upper),
                ),
                None,
            )
        } else {
            // Create recommendation for out-of-range ratio
            let rec = Recommendation {
                category: RecommendationCategory::ParameterTuning,
                description: format!(
                    "{} ratio {:.3} outside target range",
                    embedder.as_str(),
                    ratio
                ),
                component: format!("{} direction_modifier", embedder.as_str()),
                current: Some(format!("{:.3}", ratio)),
                recommended: Some(format!("{:.2}", EXPECTED_ASYMMETRIC_RATIO)),
                reason: if ratio > upper {
                    "Reduce direction_modifier from 1.2 to ~1.1".to_string()
                } else {
                    "Increase direction_modifier from 1.2 to ~1.3".to_string()
                },
            };
            (
                check.fail(
                    &format!("{:.3}", ratio),
                    &format!("[{:.2}, {:.2}]", lower, upper),
                ),
                Some(rec),
            )
        };

        (check, metrics, recommendation)
    }

    fn test_vector_distinctness(
        embedder: EmbedderName,
        percentage: f64,
    ) -> (ValidationCheck, Option<Recommendation>) {
        let check = ValidationCheck::new(
            &format!("{}_vector_distinctness", embedder.as_str()),
            &format!(
                "{} vector distinctness > {}%",
                embedder.as_str(),
                MIN_DISTINCTNESS * 100.0
            ),
        )
        .with_priority(CheckPriority::High);

        let (check, recommendation) = if percentage >= MIN_DISTINCTNESS {
            (
                check.pass(
                    &format!("{:.1}%", percentage * 100.0),
                    &format!("> {:.0}%", MIN_DISTINCTNESS * 100.0),
                ),
                None,
            )
        } else {
            let rec = Recommendation {
                category: RecommendationCategory::Performance,
                description: format!(
                    "{} forward/reverse vectors too similar ({:.1}% distinct)",
                    embedder.as_str(),
                    percentage * 100.0
                ),
                component: embedder.as_str().to_string(),
                current: Some(format!("{:.1}%", percentage * 100.0)),
                recommended: Some(format!("> {:.0}%", MIN_DISTINCTNESS * 100.0)),
                reason: "Low distinctness reduces asymmetric effectiveness".to_string(),
            };
            (
                check.warning(
                    &format!("{:.1}%", percentage * 100.0),
                    &format!("> {:.0}%", MIN_DISTINCTNESS * 100.0),
                ),
                Some(rec),
            )
        };

        (check, recommendation)
    }

    /// Compute asymmetric ratio from forward/backward similarity scores.
    pub fn compute_asymmetric_ratio(forward_scores: &[f64], backward_scores: &[f64]) -> f64 {
        if forward_scores.is_empty() || backward_scores.is_empty() {
            return 0.0;
        }

        let forward_avg: f64 = forward_scores.iter().sum::<f64>() / forward_scores.len() as f64;
        let backward_avg: f64 = backward_scores.iter().sum::<f64>() / backward_scores.len() as f64;

        if backward_avg < f64::EPSILON {
            return 0.0;
        }

        forward_avg / backward_avg
    }

    /// Compute vector distinctness (percentage of pairs with sim < 0.95).
    pub fn compute_distinctness(similarities: &[f64], threshold: f64) -> f64 {
        if similarities.is_empty() {
            return 0.0;
        }

        let distinct_count = similarities.iter().filter(|&&sim| sim < threshold).count();
        distinct_count as f64 / similarities.len() as f64
    }
}

#[cfg(all(test, feature = "benchmark-tests"))]
mod tests {
    use super::*;

    #[test]
    fn test_asymmetric_validation_config() {
        let result = AsymmetricValidator::validate(None, None);
        assert!(result.all_passed);
        assert!(!result.checks.is_empty());
    }

    #[test]
    fn test_asymmetric_ratio_computation() {
        let forward = vec![0.9, 0.85, 0.88];
        let backward = vec![0.6, 0.55, 0.58];
        let ratio = AsymmetricValidator::compute_asymmetric_ratio(&forward, &backward);
        assert!((ratio - 1.52).abs() < 0.1); // ~1.5 ratio
    }

    #[test]
    fn test_distinctness_computation() {
        let sims = vec![0.9, 0.92, 0.96, 0.88, 0.94];
        let distinctness = AsymmetricValidator::compute_distinctness(&sims, 0.95);
        assert!((distinctness - 0.8).abs() < f64::EPSILON); // 4/5 = 80% below 0.95
    }

    #[test]
    fn test_with_measured_ratios() {
        let mut ratios = HashMap::new();
        ratios.insert(EmbedderName::E5Causal, 1.52);
        ratios.insert(EmbedderName::E8Graph, 1.48);
        ratios.insert(EmbedderName::E10Multimodal, 1.55);

        let result = AsymmetricValidator::validate(Some(&ratios), None);
        assert!(result.all_passed);
    }

    #[test]
    fn test_out_of_range_ratio() {
        let mut ratios = HashMap::new();
        ratios.insert(EmbedderName::E5Causal, 1.8); // Outside tolerance

        let result = AsymmetricValidator::validate(Some(&ratios), None);
        assert!(!result.all_passed);
        assert!(!result.recommendations.is_empty());
    }
}
