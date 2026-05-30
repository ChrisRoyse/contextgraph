use crate::contradiction::{
    detect_multi_head_contradiction, ContradictionDecision, ContradictionThresholds,
};
use crate::types::{ConformalInterval, PredictedFailureMode, PredictedWorks, Severity, Verdict};

#[derive(Debug, Clone, Copy)]
pub struct VerdictAssemblyInput<'a> {
    pub contradiction_cell_id: Option<&'a str>,
    pub contradiction_thresholds: Option<&'a ContradictionThresholds>,
    pub oracle_pass_confidence: f32,
    pub failure_modes: &'a [PredictedFailureMode],
    pub predicted_failed_test_count: usize,
    pub predicted_works: &'a [PredictedWorks],
    pub security_concern_count: usize,
    pub guard_violation_count: usize,
    pub ood_score: f32,
    pub confidence_interval: &'a ConformalInterval,
    pub pass_threshold: f32,
    pub ood_threshold: f32,
    pub interval_width_threshold: f32,
    pub safety_constraint_violation_count: usize,
    pub objective_total_cost: f32,
    pub objective_cost_ceiling: f32,
    pub constellation_verdict_pressure: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VerdictAssemblyOutput {
    pub verdict: Verdict,
    pub contradiction: ContradictionDecision,
}

pub fn assemble_verdict(input: VerdictAssemblyInput<'_>) -> Verdict {
    assemble_verdict_with_evidence(input).verdict
}

pub fn assemble_verdict_with_evidence(input: VerdictAssemblyInput<'_>) -> VerdictAssemblyOutput {
    let active_security_concern_count = if crate::q4_trust_gate::Q4_DOCTRINE_FREEZE_ACTIVE {
        0
    } else {
        input.security_concern_count
    };
    let high_severity_concerns = input
        .failure_modes
        .iter()
        .filter(|mode| {
            matches!(mode.severity, Severity::High | Severity::Critical) && mode.confidence > 0.6
        })
        .count() as u32;
    let contradiction = detect_multi_head_contradiction(
        input.contradiction_cell_id,
        input.oracle_pass_confidence,
        high_severity_concerns,
        active_security_concern_count as u32,
        input.contradiction_thresholds,
    )
    .unwrap_or_else(|err| ContradictionDecision {
        kind: crate::contradiction::ContradictionDecisionKind::ThresholdMissing,
        reason: format!(
            "{}:{}",
            crate::contradiction::CONTRADICTION_CALIBRATION_INVALID,
            err.code()
        ),
        cell_id: input.contradiction_cell_id.map(ToString::to_string),
        oracle_pass_confidence: input.oracle_pass_confidence,
        high_severity_failure_count: high_severity_concerns,
        security_concern_count: active_security_concern_count as u32,
        tau_oracle: None,
        tau_failure_count: None,
        verdict_override: Some(Verdict::Abstain),
    });

    if input.safety_constraint_violation_count > 0 {
        return VerdictAssemblyOutput {
            verdict: Verdict::GuardRejected,
            contradiction,
        };
    }
    if input.guard_violation_count > 0 {
        return VerdictAssemblyOutput {
            verdict: Verdict::GuardRejected,
            contradiction,
        };
    }
    if input.ood_score >= input.ood_threshold {
        return VerdictAssemblyOutput {
            verdict: Verdict::OutOfDistribution,
            contradiction,
        };
    }
    if input.confidence_interval.width() > input.interval_width_threshold {
        return VerdictAssemblyOutput {
            verdict: Verdict::Abstain,
            contradiction,
        };
    }

    if let Some(verdict) = contradiction.verdict_override {
        return VerdictAssemblyOutput {
            verdict,
            contradiction,
        };
    }
    if input.constellation_verdict_pressure >= 0.85 {
        return VerdictAssemblyOutput {
            verdict: Verdict::Abstain,
            contradiction,
        };
    }

    if input.oracle_pass_confidence >= input.pass_threshold
        && input.confidence_interval.lower >= input.pass_threshold
        && input.predicted_failed_test_count == 0
        && input
            .predicted_works
            .iter()
            .any(PredictedWorks::is_high_confidence)
        && high_severity_concerns == 0
        && active_security_concern_count == 0
        && input.objective_total_cost <= input.objective_cost_ceiling
    {
        VerdictAssemblyOutput {
            verdict: Verdict::Pass,
            contradiction,
        }
    } else {
        VerdictAssemblyOutput {
            verdict: Verdict::Fail,
            contradiction,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{
        ChunkId, ConformalInterval, ConformalMethod, EmbedderId, FailureModeClass, PredictedWorks,
        RootCauseClass,
    };

    fn interval(lower: f32, upper: f32) -> ConformalInterval {
        ConformalInterval {
            lower,
            upper,
            method: ConformalMethod::SplitConformal,
            coverage_target: 0.9,
            empirical_coverage: 0.9,
        }
    }

    #[test]
    fn contradiction_abstains_instead_of_pass() {
        let failure = PredictedFailureMode {
            failure_class: FailureModeClass::WrongAlgorithm,
            chunk: ChunkId("src/lib.rs#0".to_string()),
            line_range: (1, 3),
            confidence: 0.91,
            severity: Severity::High,
            explanation: "oracle and failure-mode heads disagree".to_string(),
            contributing_embedders: Vec::new(),
            root_cause_class: RootCauseClass::LogicError,
        };
        let verdict = assemble_verdict(VerdictAssemblyInput {
            contradiction_cell_id: None,
            contradiction_thresholds: None,
            oracle_pass_confidence: 0.92,
            failure_modes: &[failure],
            predicted_failed_test_count: 0,
            predicted_works: &[],
            security_concern_count: 0,
            guard_violation_count: 0,
            ood_score: 0.1,
            confidence_interval: &interval(0.9, 0.94),
            pass_threshold: 0.8,
            ood_threshold: 0.9,
            interval_width_threshold: 0.5,
            safety_constraint_violation_count: 0,
            objective_total_cost: 0.05,
            objective_cost_ceiling: 0.65,
            constellation_verdict_pressure: 0.0,
        });
        assert_eq!(verdict, Verdict::Abstain);
    }

    #[test]
    fn q4_security_count_is_ignored_under_doctrine_freeze() {
        let work = PredictedWorks {
            chunk: ChunkId("src/lib.py#0".to_string()),
            line_range: (1, 3),
            claim: "binary_search is predicted to preserve behavior".to_string(),
            confidence: 0.92,
            supporting_embedders: vec![EmbedderId("E_AST".to_string())],
            similar_known_good_exemplars: Vec::new(),
            evidence_strength: 0.86,
        };
        let verdict = assemble_verdict(VerdictAssemblyInput {
            contradiction_cell_id: None,
            contradiction_thresholds: None,
            oracle_pass_confidence: 0.95,
            failure_modes: &[],
            predicted_failed_test_count: 0,
            predicted_works: &[work],
            security_concern_count: 1,
            guard_violation_count: 0,
            ood_score: 0.1,
            confidence_interval: &interval(0.9, 0.94),
            pass_threshold: 0.8,
            ood_threshold: 0.9,
            interval_width_threshold: 0.5,
            safety_constraint_violation_count: 0,
            objective_total_cost: 0.05,
            objective_cost_ceiling: 0.65,
            constellation_verdict_pressure: 0.0,
        });
        assert_eq!(verdict, Verdict::Pass);
    }

    #[test]
    fn hard_safety_constraint_rejects_before_pass() {
        let verdict = assemble_verdict(VerdictAssemblyInput {
            contradiction_cell_id: None,
            contradiction_thresholds: None,
            oracle_pass_confidence: 0.99,
            failure_modes: &[],
            predicted_failed_test_count: 0,
            predicted_works: &[],
            security_concern_count: 0,
            guard_violation_count: 0,
            ood_score: 0.1,
            confidence_interval: &interval(0.95, 0.99),
            pass_threshold: 0.8,
            ood_threshold: 0.9,
            interval_width_threshold: 0.5,
            safety_constraint_violation_count: 1,
            objective_total_cost: 0.01,
            objective_cost_ceiling: 0.65,
            constellation_verdict_pressure: 0.0,
        });
        assert_eq!(verdict, Verdict::GuardRejected);
    }

    #[test]
    fn guard_violation_takes_precedence_over_generic_ood() {
        let verdict = assemble_verdict(VerdictAssemblyInput {
            contradiction_cell_id: None,
            contradiction_thresholds: None,
            oracle_pass_confidence: 0.99,
            failure_modes: &[],
            predicted_failed_test_count: 0,
            predicted_works: &[],
            security_concern_count: 0,
            guard_violation_count: 1,
            ood_score: 0.99,
            confidence_interval: &interval(0.95, 0.99),
            pass_threshold: 0.8,
            ood_threshold: 0.5,
            interval_width_threshold: 0.5,
            safety_constraint_violation_count: 0,
            objective_total_cost: 0.01,
            objective_cost_ceiling: 0.65,
            constellation_verdict_pressure: 0.0,
        });
        assert_eq!(verdict, Verdict::GuardRejected);
    }

    #[test]
    fn high_objective_cost_blocks_pass() {
        let verdict = assemble_verdict(VerdictAssemblyInput {
            contradiction_cell_id: None,
            contradiction_thresholds: None,
            oracle_pass_confidence: 0.99,
            failure_modes: &[],
            predicted_failed_test_count: 0,
            predicted_works: &[],
            security_concern_count: 0,
            guard_violation_count: 0,
            ood_score: 0.1,
            confidence_interval: &interval(0.95, 0.99),
            pass_threshold: 0.8,
            ood_threshold: 0.9,
            interval_width_threshold: 0.5,
            safety_constraint_violation_count: 0,
            objective_total_cost: 0.90,
            objective_cost_ceiling: 0.65,
            constellation_verdict_pressure: 0.0,
        });
        assert_eq!(verdict, Verdict::Fail);
    }

    #[test]
    fn constellation_pressure_abstains_before_false_pass() {
        let work = PredictedWorks {
            chunk: ChunkId("src/lib.py#0".to_string()),
            line_range: (1, 3),
            claim: "code is predicted to work but slot-pair evidence disagrees".to_string(),
            confidence: 0.92,
            supporting_embedders: vec![EmbedderId("E_AST".to_string())],
            similar_known_good_exemplars: Vec::new(),
            evidence_strength: 0.86,
        };
        let verdict = assemble_verdict(VerdictAssemblyInput {
            contradiction_cell_id: None,
            contradiction_thresholds: None,
            oracle_pass_confidence: 0.99,
            failure_modes: &[],
            predicted_failed_test_count: 0,
            predicted_works: &[work],
            security_concern_count: 0,
            guard_violation_count: 0,
            ood_score: 0.1,
            confidence_interval: &interval(0.95, 0.99),
            pass_threshold: 0.8,
            ood_threshold: 0.9,
            interval_width_threshold: 0.5,
            safety_constraint_violation_count: 0,
            objective_total_cost: 0.01,
            objective_cost_ceiling: 0.65,
            constellation_verdict_pressure: 0.90,
        });
        assert_eq!(verdict, Verdict::Abstain);
    }

    #[test]
    fn pass_requires_high_confidence_predicted_works() {
        let no_positive_evidence = assemble_verdict(VerdictAssemblyInput {
            contradiction_cell_id: None,
            contradiction_thresholds: None,
            oracle_pass_confidence: 0.99,
            failure_modes: &[],
            predicted_failed_test_count: 0,
            predicted_works: &[],
            security_concern_count: 0,
            guard_violation_count: 0,
            ood_score: 0.1,
            confidence_interval: &interval(0.95, 0.99),
            pass_threshold: 0.8,
            ood_threshold: 0.9,
            interval_width_threshold: 0.5,
            safety_constraint_violation_count: 0,
            objective_total_cost: 0.01,
            objective_cost_ceiling: 0.65,
            constellation_verdict_pressure: 0.0,
        });
        assert_eq!(no_positive_evidence, Verdict::Fail);

        let work = PredictedWorks {
            chunk: ChunkId("src/lib.py#0".to_string()),
            line_range: (1, 3),
            claim: "binary_search is predicted to preserve behavior".to_string(),
            confidence: 0.92,
            supporting_embedders: vec![EmbedderId("E_AST".to_string())],
            similar_known_good_exemplars: Vec::new(),
            evidence_strength: 0.86,
        };
        let positive_evidence = assemble_verdict(VerdictAssemblyInput {
            contradiction_cell_id: None,
            contradiction_thresholds: None,
            oracle_pass_confidence: 0.99,
            failure_modes: &[],
            predicted_failed_test_count: 0,
            predicted_works: &[work],
            security_concern_count: 0,
            guard_violation_count: 0,
            ood_score: 0.1,
            confidence_interval: &interval(0.95, 0.99),
            pass_threshold: 0.8,
            ood_threshold: 0.9,
            interval_width_threshold: 0.5,
            safety_constraint_violation_count: 0,
            objective_total_cost: 0.01,
            objective_cost_ceiling: 0.65,
            constellation_verdict_pressure: 0.0,
        });
        assert_eq!(positive_evidence, Verdict::Pass);
    }

    /// #804 regression: a wide confidence interval is insufficient calibration
    /// evidence, not an OOD detector hit. Setting the interval threshold tighter
    /// than the interval width now produces Abstain while true OOD still uses
    /// the OOD score.
    #[test]
    fn configurable_interval_width_threshold_abstains_without_ood() {
        let base = VerdictAssemblyInput {
            contradiction_cell_id: None,
            contradiction_thresholds: None,
            oracle_pass_confidence: 0.92,
            failure_modes: &[],
            predicted_failed_test_count: 0,
            predicted_works: &[],
            security_concern_count: 0,
            guard_violation_count: 0,
            ood_score: 0.05,
            // Cold-start fingerprint: width 1.0.
            confidence_interval: &interval(0.0, 1.0),
            pass_threshold: 0.8,
            ood_threshold: 0.5,
            interval_width_threshold: 0.8,
            safety_constraint_violation_count: 0,
            objective_total_cost: 0.05,
            objective_cost_ceiling: 0.65,
            constellation_verdict_pressure: 0.0,
        };
        assert_eq!(assemble_verdict(base), Verdict::Abstain);
        // Looser threshold 1.5 > width 1.0 -> falls through. With no
        // contradiction / safety / predicted-failure issues, this fixture
        // does not produce Pass (no predicted_works), so the trailing arm
        // selects `Verdict::Fail`.
        let loose = VerdictAssemblyInput {
            interval_width_threshold: 1.5,
            ..base
        };
        assert_ne!(assemble_verdict(loose), Verdict::OutOfDistribution);
    }

    #[test]
    fn ood_score_still_gates_ood_verdict() {
        let input = VerdictAssemblyInput {
            contradiction_cell_id: None,
            contradiction_thresholds: None,
            oracle_pass_confidence: 0.92,
            failure_modes: &[],
            predicted_failed_test_count: 0,
            predicted_works: &[],
            security_concern_count: 0,
            guard_violation_count: 0,
            ood_score: 0.91,
            confidence_interval: &interval(0.80, 0.82),
            pass_threshold: 0.8,
            ood_threshold: 0.9,
            interval_width_threshold: 0.8,
            safety_constraint_violation_count: 0,
            objective_total_cost: 0.05,
            objective_cost_ceiling: 0.65,
            constellation_verdict_pressure: 0.0,
        };

        assert_eq!(assemble_verdict(input), Verdict::OutOfDistribution);
    }
}
