use crate::types::{ConformalInterval, PredictedFailureMode, PredictedWorks, Severity, Verdict};

#[derive(Debug, Clone, Copy)]
pub struct VerdictAssemblyInput<'a> {
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
}

pub fn assemble_verdict(input: VerdictAssemblyInput<'_>) -> Verdict {
    if input.safety_constraint_violation_count > 0 {
        return Verdict::GuardRejected;
    }
    if input.ood_score >= input.ood_threshold
        || input.confidence_interval.width() > input.interval_width_threshold
    {
        return Verdict::OutOfDistribution;
    }
    if input.guard_violation_count > 0 {
        return Verdict::GuardRejected;
    }

    let high_severity_concerns = input
        .failure_modes
        .iter()
        .filter(|mode| {
            matches!(mode.severity, Severity::High | Severity::Critical) && mode.confidence > 0.6
        })
        .count();
    if input.oracle_pass_confidence > 0.85
        && (high_severity_concerns > 0 || input.security_concern_count > 0)
    {
        return Verdict::Abstain;
    }

    if input.oracle_pass_confidence >= input.pass_threshold
        && input.confidence_interval.lower >= input.pass_threshold
        && input.predicted_failed_test_count == 0
        && input
            .predicted_works
            .iter()
            .any(PredictedWorks::is_high_confidence)
        && high_severity_concerns == 0
        && input.security_concern_count == 0
        && input.objective_total_cost <= input.objective_cost_ceiling
    {
        Verdict::Pass
    } else {
        Verdict::Fail
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
        });
        assert_eq!(verdict, Verdict::Abstain);
    }

    #[test]
    fn security_concern_abstains_instead_of_false_confident_pass() {
        let verdict = assemble_verdict(VerdictAssemblyInput {
            oracle_pass_confidence: 0.95,
            failure_modes: &[],
            predicted_failed_test_count: 0,
            predicted_works: &[],
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
        });
        assert_eq!(verdict, Verdict::Abstain);
    }

    #[test]
    fn hard_safety_constraint_rejects_before_pass() {
        let verdict = assemble_verdict(VerdictAssemblyInput {
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
        });
        assert_eq!(verdict, Verdict::GuardRejected);
    }

    #[test]
    fn high_objective_cost_blocks_pass() {
        let verdict = assemble_verdict(VerdictAssemblyInput {
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
        });
        assert_eq!(verdict, Verdict::Fail);
    }

    #[test]
    fn pass_requires_high_confidence_predicted_works() {
        let no_positive_evidence = assemble_verdict(VerdictAssemblyInput {
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
        });
        assert_eq!(positive_evidence, Verdict::Pass);
    }
}
