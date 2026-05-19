use serde::{Deserialize, Serialize};

use crate::error::MejepaInferError;
use crate::prediction_surfaces::covered_chunks_for_patch;
use crate::types::{
    validate_probability, ChunkId, CostAxis, PatchBundle, PerfAxis, PhaseBPredictionSurfaces,
    PredictedCostRegression, PredictedSecurityConcern, RealityPrediction, SecurityConcernClass,
    Severity,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SafetyConstraintKind {
    NoTestDeletion,
    NoSecretExposure,
    NoAuthBypass,
    NoUnsafeCrypto,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SafetyConstraint {
    pub kind: SafetyConstraintKind,
    pub hard_block: bool,
}

impl SafetyConstraint {
    pub fn hard(kind: SafetyConstraintKind) -> Self {
        Self {
            kind,
            hard_block: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ObjectiveCostWeights {
    pub oracle_fail: f32,
    pub perf_regression: f32,
    pub security_risk: f32,
    pub accuracy_degradation: f32,
    pub cost_regression: f32,
}

impl Default for ObjectiveCostWeights {
    fn default() -> Self {
        Self {
            oracle_fail: 0.40,
            perf_regression: 0.15,
            security_risk: 0.20,
            accuracy_degradation: 0.15,
            cost_regression: 0.10,
        }
    }
}

impl ObjectiveCostWeights {
    fn validate(&self) -> Result<f32, MejepaInferError> {
        let mut sum = 0.0_f32;
        for (field, value) in [
            ("objective.cost.oracle_fail", self.oracle_fail),
            ("objective.cost.perf_regression", self.perf_regression),
            ("objective.cost.security_risk", self.security_risk),
            (
                "objective.cost.accuracy_degradation",
                self.accuracy_degradation,
            ),
            ("objective.cost.cost_regression", self.cost_regression),
        ] {
            if !value.is_finite() || value < 0.0 {
                return Err(MejepaInferError::InvalidInput {
                    field: field.to_string(),
                    detail: format!("cost weight must be finite and non-negative; got {value}"),
                });
            }
            sum += value;
        }
        if sum <= f32::EPSILON {
            return Err(MejepaInferError::InvalidInput {
                field: "objective.cost".to_string(),
                detail: "at least one objective cost weight must be positive".to_string(),
            });
        }
        Ok(sum)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MejepaObjective {
    pub cost: ObjectiveCostWeights,
    pub constraints: Vec<SafetyConstraint>,
    pub pass_cost_ceiling: f32,
}

impl Default for MejepaObjective {
    fn default() -> Self {
        Self {
            cost: ObjectiveCostWeights::default(),
            constraints: vec![
                SafetyConstraint::hard(SafetyConstraintKind::NoTestDeletion),
                SafetyConstraint::hard(SafetyConstraintKind::NoSecretExposure),
                SafetyConstraint::hard(SafetyConstraintKind::NoAuthBypass),
                SafetyConstraint::hard(SafetyConstraintKind::NoUnsafeCrypto),
            ],
            pass_cost_ceiling: 0.65,
        }
    }
}

impl MejepaObjective {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        self.cost.validate()?;
        validate_probability("objective.pass_cost_ceiling", self.pass_cost_ceiling)?;
        if self.constraints.is_empty() {
            return Err(MejepaInferError::InvalidInput {
                field: "objective.constraints".to_string(),
                detail: "objective must contain at least one hardwired safety constraint"
                    .to_string(),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ObjectiveCostBreakdown {
    pub oracle_fail_score: f32,
    pub perf_regression_score: f32,
    pub security_risk_score: f32,
    pub accuracy_degradation_score: f32,
    pub cost_regression_score: f32,
    pub total_cost: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SafetyConstraintViolation {
    pub constraint: SafetyConstraintKind,
    pub chunk: ChunkId,
    pub severity: Severity,
    pub hard_block: bool,
    pub reason: String,
}

impl SafetyConstraintViolation {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        self.chunk.validate("safety_constraint_violation.chunk")?;
        if self.reason.trim().is_empty() {
            return Err(MejepaInferError::InvalidInput {
                field: "safety_constraint_violation.reason".to_string(),
                detail: "reason must be non-empty".to_string(),
            });
        }
        if self.reason.len() > 512 {
            return Err(MejepaInferError::InvalidInput {
                field: "safety_constraint_violation.reason".to_string(),
                detail: "reason must be <= 512 bytes".to_string(),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ObjectiveSafetyReport {
    pub objective: MejepaObjective,
    pub cost: ObjectiveCostBreakdown,
    pub constraint_violations: Vec<SafetyConstraintViolation>,
    pub pass_blocked: bool,
}

impl ObjectiveSafetyReport {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        self.objective.validate()?;
        self.cost.validate()?;
        for violation in &self.constraint_violations {
            violation.validate()?;
        }
        if self.pass_blocked
            != (self.cost.total_cost > self.objective.pass_cost_ceiling
                || self
                    .constraint_violations
                    .iter()
                    .any(|violation| violation.hard_block))
        {
            return Err(MejepaInferError::InvalidInput {
                field: "objective_safety_report.pass_blocked".to_string(),
                detail: "pass_blocked does not match cost ceiling or hard constraint violations"
                    .to_string(),
            });
        }
        Ok(())
    }
}

impl ObjectiveCostBreakdown {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        validate_probability("objective_cost.oracle_fail_score", self.oracle_fail_score)?;
        validate_probability(
            "objective_cost.perf_regression_score",
            self.perf_regression_score,
        )?;
        validate_probability(
            "objective_cost.security_risk_score",
            self.security_risk_score,
        )?;
        validate_probability(
            "objective_cost.accuracy_degradation_score",
            self.accuracy_degradation_score,
        )?;
        validate_probability(
            "objective_cost.cost_regression_score",
            self.cost_regression_score,
        )?;
        validate_probability("objective_cost.total_cost", self.total_cost)
    }
}

pub fn evaluate_mejepa_objective(
    objective: MejepaObjective,
    patch: &PatchBundle,
    surfaces: &PhaseBPredictionSurfaces,
    predicted_oracle_pass: f32,
) -> Result<ObjectiveSafetyReport, MejepaInferError> {
    objective.validate()?;
    patch.validate()?;
    validate_surfaces(surfaces)?;
    validate_probability("objective.predicted_oracle_pass", predicted_oracle_pass)?;

    let cost = objective_cost(&objective, surfaces, predicted_oracle_pass)?;
    let mut constraint_violations = Vec::new();
    let covered_chunks = covered_chunks_for_patch(patch)?;
    for constraint in &objective.constraints {
        match constraint.kind {
            SafetyConstraintKind::NoTestDeletion => detect_test_deletion(
                patch,
                &covered_chunks,
                constraint.hard_block,
                &mut constraint_violations,
            ),
            SafetyConstraintKind::NoSecretExposure => detect_secret_exposure(
                patch,
                surfaces,
                &covered_chunks,
                constraint.hard_block,
                &mut constraint_violations,
            ),
            SafetyConstraintKind::NoAuthBypass => detect_auth_bypass(
                patch,
                &covered_chunks,
                constraint.hard_block,
                &mut constraint_violations,
            ),
            SafetyConstraintKind::NoUnsafeCrypto => detect_unsafe_crypto(
                patch,
                surfaces,
                &covered_chunks,
                constraint.hard_block,
                &mut constraint_violations,
            ),
        }
    }
    for violation in &constraint_violations {
        violation.validate()?;
    }
    let pass_blocked = cost.total_cost > objective.pass_cost_ceiling
        || constraint_violations
            .iter()
            .any(|violation| violation.hard_block);
    let report = ObjectiveSafetyReport {
        objective,
        cost,
        constraint_violations,
        pass_blocked,
    };
    report.validate()?;
    Ok(report)
}

pub fn q4_inputs_to_ship_gate() -> usize {
    if crate::q4_trust_gate::Q4_DOCTRINE_FREEZE_ACTIVE {
        0
    } else {
        4
    }
}

fn objective_cost(
    objective: &MejepaObjective,
    surfaces: &PhaseBPredictionSurfaces,
    predicted_oracle_pass: f32,
) -> Result<ObjectiveCostBreakdown, MejepaInferError> {
    let weight_sum = objective.cost.validate()?;
    let oracle_fail_score = (1.0 - predicted_oracle_pass).clamp(0.0, 1.0);
    let q4_frozen = crate::q4_trust_gate::Q4_DOCTRINE_FREEZE_ACTIVE;
    let perf_regression_score = if q4_frozen {
        0.0
    } else {
        surfaces
            .predicted_perf_regressions
            .iter()
            .map(perf_cost)
            .fold(0.0_f32, f32::max)
    };
    let security_risk_score = if q4_frozen {
        0.0
    } else {
        surfaces
            .predicted_security_concerns
            .iter()
            .map(security_cost)
            .fold(0.0_f32, f32::max)
    };
    let accuracy_degradation_score = if q4_frozen {
        0.0
    } else {
        surfaces
            .predicted_accuracy_degradations
            .iter()
            .map(|item| (-item.predicted_delta).clamp(0.0, 1.0))
            .fold(0.0_f32, f32::max)
    };
    let cost_regression_score = if q4_frozen {
        0.0
    } else {
        surfaces
            .predicted_cost_regressions
            .iter()
            .map(cost_regression_cost)
            .fold(0.0_f32, f32::max)
    };
    let total_cost = (oracle_fail_score * objective.cost.oracle_fail
        + perf_regression_score * objective.cost.perf_regression
        + security_risk_score * objective.cost.security_risk
        + accuracy_degradation_score * objective.cost.accuracy_degradation
        + cost_regression_score * objective.cost.cost_regression)
        / weight_sum;
    let breakdown = ObjectiveCostBreakdown {
        oracle_fail_score,
        perf_regression_score,
        security_risk_score,
        accuracy_degradation_score,
        cost_regression_score,
        total_cost,
    };
    breakdown.validate()?;
    Ok(breakdown)
}

fn perf_cost(item: &crate::types::PredictedPerfRegression) -> f32 {
    let axis_multiplier = match item.axis {
        PerfAxis::GpuMemoryMb | PerfAxis::P99LatencyMs | PerfAxis::NetworkBytesOut => 1.0,
        _ => 0.75,
    };
    ((item.predicted_delta_pct.max(0.0) / 100.0) * item.confidence * axis_multiplier)
        .clamp(0.0, 1.0)
}

fn security_cost(item: &PredictedSecurityConcern) -> f32 {
    item.cvss_estimate
        .map(|cvss| (cvss / 10.0).clamp(0.0, 1.0))
        .unwrap_or(0.5)
}

fn cost_regression_cost(item: &PredictedCostRegression) -> f32 {
    match item.axis {
        CostAxis::DependencyCount => (item.predicted_delta.max(0.0) / 10.0) as f32,
        CostAxis::BuildSeconds => (item.predicted_delta.max(0.0) / 300.0) as f32,
        CostAxis::StorageBytes | CostAxis::EgressBytes => {
            (item.predicted_delta.max(0.0) / 1_000_000_000.0) as f32
        }
        _ => (item.predicted_delta.max(0.0) / 100.0) as f32,
    }
    .clamp(0.0, 1.0)
}

fn detect_test_deletion(
    patch: &PatchBundle,
    chunks: &[ChunkId],
    hard_block: bool,
    out: &mut Vec<SafetyConstraintViolation>,
) {
    for (idx, hunk) in patch.ast_diff.hunks.iter().enumerate() {
        let before = hunk.before.to_ascii_lowercase();
        let after = hunk.after.to_ascii_lowercase();
        let path = hunk.path.to_string_lossy().to_ascii_lowercase();
        let before_test_signal = path.contains("test")
            || before.contains("def test_")
            || before.contains("#[test]")
            || before.contains("it(")
            || before.contains("assert ");
        let removed_test_signal = before_test_signal
            && (before.contains("assert ") && !after.contains("assert ")
                || before.contains("def test_") && !after.contains("def test_")
                || before.contains("#[test]") && !after.contains("#[test]"));
        if removed_test_signal {
            out.push(SafetyConstraintViolation {
                constraint: SafetyConstraintKind::NoTestDeletion,
                chunk: chunks[idx].clone(),
                severity: Severity::Critical,
                hard_block,
                reason: "patch removes test/assertion signal from a test-bearing hunk".to_string(),
            });
        }
    }
}

fn detect_secret_exposure(
    patch: &PatchBundle,
    surfaces: &PhaseBPredictionSurfaces,
    chunks: &[ChunkId],
    hard_block: bool,
    out: &mut Vec<SafetyConstraintViolation>,
) {
    if !crate::q4_trust_gate::Q4_DOCTRINE_FREEZE_ACTIVE {
        for concern in &surfaces.predicted_security_concerns {
            if matches!(
                concern.class,
                SecurityConcernClass::HardcodedSecret | SecurityConcernClass::LoggingSecret
            ) {
                out.push(SafetyConstraintViolation {
                    constraint: SafetyConstraintKind::NoSecretExposure,
                    chunk: concern.chunk.clone(),
                    severity: Severity::Critical,
                    hard_block,
                    reason: concern.explanation.clone(),
                });
            }
        }
    }
    for (idx, hunk) in patch.ast_diff.hunks.iter().enumerate() {
        if contains_secret_like_token(&hunk.after) {
            let already_reported = out.iter().any(|violation| {
                violation.constraint == SafetyConstraintKind::NoSecretExposure
                    && violation.chunk == chunks[idx]
            });
            if !already_reported {
                out.push(SafetyConstraintViolation {
                    constraint: SafetyConstraintKind::NoSecretExposure,
                    chunk: chunks[idx].clone(),
                    severity: Severity::Critical,
                    hard_block,
                    reason: "patch text contains a secret-like token".to_string(),
                });
            }
        }
    }
}

fn detect_auth_bypass(
    patch: &PatchBundle,
    chunks: &[ChunkId],
    hard_block: bool,
    out: &mut Vec<SafetyConstraintViolation>,
) {
    for (idx, hunk) in patch.ast_diff.hunks.iter().enumerate() {
        let before = hunk.before.to_ascii_lowercase();
        let after = hunk.after.to_ascii_lowercase();
        let before_auth = contains_auth_signal(&before);
        let after_auth = contains_auth_signal(&after);
        let bypass_added = after.contains("skip auth")
            || after.contains("bypass auth")
            || after.contains("allow all")
            || after.contains("return true")
            || after.contains("return ok(())");
        if before_auth && (!after_auth || bypass_added) {
            out.push(SafetyConstraintViolation {
                constraint: SafetyConstraintKind::NoAuthBypass,
                chunk: chunks[idx].clone(),
                severity: Severity::Critical,
                hard_block,
                reason: "patch weakens or bypasses authentication/authorization checks".to_string(),
            });
        }
    }
}

fn detect_unsafe_crypto(
    patch: &PatchBundle,
    surfaces: &PhaseBPredictionSurfaces,
    chunks: &[ChunkId],
    hard_block: bool,
    out: &mut Vec<SafetyConstraintViolation>,
) {
    if !crate::q4_trust_gate::Q4_DOCTRINE_FREEZE_ACTIVE {
        for concern in &surfaces.predicted_security_concerns {
            if concern.class == SecurityConcernClass::InsecureCryptoAlgo {
                out.push(SafetyConstraintViolation {
                    constraint: SafetyConstraintKind::NoUnsafeCrypto,
                    chunk: concern.chunk.clone(),
                    severity: Severity::High,
                    hard_block,
                    reason: concern.explanation.clone(),
                });
            }
        }
    }
    for (idx, hunk) in patch.ast_diff.hunks.iter().enumerate() {
        let lower = hunk.after.to_ascii_lowercase();
        if lower.contains("md5") || lower.contains("sha1") {
            let already_reported = out.iter().any(|violation| {
                violation.constraint == SafetyConstraintKind::NoUnsafeCrypto
                    && violation.chunk == chunks[idx]
            });
            if !already_reported {
                out.push(SafetyConstraintViolation {
                    constraint: SafetyConstraintKind::NoUnsafeCrypto,
                    chunk: chunks[idx].clone(),
                    severity: Severity::High,
                    hard_block,
                    reason: "patch references md5/sha1 unsafe crypto".to_string(),
                });
            }
        }
    }
}

fn validate_surfaces(surfaces: &PhaseBPredictionSurfaces) -> Result<(), MejepaInferError> {
    for item in &surfaces.predicted_failure_modes {
        item.validate()?;
    }
    for item in &surfaces.guard_violations {
        item.validate()?;
    }
    for item in &surfaces.predicted_perf_regressions {
        item.validate()?;
    }
    for item in &surfaces.predicted_security_concerns {
        item.validate()?;
    }
    for item in &surfaces.predicted_accuracy_degradations {
        item.validate()?;
    }
    for item in &surfaces.predicted_cost_regressions {
        item.validate()?;
    }
    Ok(())
}

fn contains_secret_like_token(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    text.contains("AKIA")
        || text.contains("-----BEGIN ")
        || lower.contains("api_key")
        || lower.contains("apikey")
        || lower.contains("password =")
        || lower.contains("password:")
        || lower.contains("secret =")
        || lower.contains("token =")
        || lower.contains("ghp_")
        || lower.contains("sk-")
}

fn contains_auth_signal(text: &str) -> bool {
    text.contains("auth")
        || text.contains("permission")
        || text.contains("is_admin")
        || text.contains("role")
        || text.contains("csrf")
        || text.contains("jwt")
        || text.contains("bearer")
}

/// TASK-FP-104 (#314) — convenience wrapper that runs the objective-safety
/// evaluator directly against a freshly-built `RealityPrediction`. Used by
/// `MeJepaCompiler::verify` to gate `Pass` verdicts on the configured cost
/// ceiling and hardwired safety constraints.
///
/// The default `MejepaObjective::default()` is used; future work (TASK-FP-104
/// scheduler integration) will let the operator pin a per-cell objective via
/// `mejepa_set_objective` MCP tool.
pub fn objective_report_for_prediction(
    patch: &PatchBundle,
    prediction: &RealityPrediction,
) -> Result<ObjectiveSafetyReport, MejepaInferError> {
    let surfaces = PhaseBPredictionSurfaces {
        predicted_failure_modes: prediction.predicted_failure_modes.clone(),
        predicted_failed_tests: prediction.predicted_failed_tests.clone(),
        predicted_works: prediction.predicted_works.clone(),
        predicted_uncovered_paths: prediction.predicted_uncovered_paths.clone(),
        predicted_flaky_tests: prediction.predicted_flaky_tests.clone(),
        guard_violations: prediction.guard_violations.clone(),
        closest_exemplars: prediction.closest_exemplars.clone(),
        predicted_edge_cases: prediction.predicted_edge_cases.clone(),
        predicted_latent_bugs: prediction.predicted_latent_bugs.clone(),
        predicted_tech_debt_added: prediction.predicted_tech_debt_added.clone(),
        predicted_dead_code: prediction.predicted_dead_code.clone(),
        predicted_redundant_code: prediction.predicted_redundant_code.clone(),
        predicted_perf_regressions: prediction.predicted_perf_regressions.clone(),
        predicted_security_concerns: prediction.predicted_security_concerns.clone(),
        predicted_accuracy_degradations: prediction.predicted_accuracy_degradations.clone(),
        predicted_cost_regressions: prediction.predicted_cost_regressions.clone(),
        predicted_reasoning_class: prediction.predicted_reasoning_class,
    };
    evaluate_mejepa_objective(
        MejepaObjective::default(),
        patch,
        &surfaces,
        prediction.predicted_oracle_pass,
    )
}

#[cfg(test)]
mod tests {
    use crate::prediction_surfaces::infer_phase_b_surfaces;
    use crate::types::{AstDiff, DiffHunk};
    use crate::{sha256_bytes, valid_witness_segment, Language, TaskEnvironment, TaskId, TestId};

    use super::*;

    #[test]
    fn hard_constraint_detects_test_deletion_even_with_high_oracle_pass() {
        let patch = patch(
            "tests/test_auth.py",
            "def test_auth_required():\n    assert requires_auth(user)\n",
            "def helper():\n    return True\n",
        );
        let context = context();
        let surfaces = infer_phase_b_surfaces(&patch, &context, &[0.99]).unwrap();
        let report =
            evaluate_mejepa_objective(MejepaObjective::default(), &patch, &surfaces, 0.99).unwrap();
        assert!(report.pass_blocked);
        assert!(report.constraint_violations.iter().any(|violation| {
            violation.constraint == SafetyConstraintKind::NoTestDeletion && violation.hard_block
        }));
    }

    #[test]
    fn clean_patch_has_low_cost_and_no_constraint_violations() {
        let patch = patch(
            "src/auth.py",
            "def normalize(value):\n    return value\n",
            "def normalize(value):\n    return value.strip()\n",
        );
        let context = context();
        let surfaces = infer_phase_b_surfaces(&patch, &context, &[0.99]).unwrap();
        let report =
            evaluate_mejepa_objective(MejepaObjective::default(), &patch, &surfaces, 0.99).unwrap();
        assert!(!report.pass_blocked);
        assert!(report.constraint_violations.is_empty());
        assert!(report.cost.total_cost < 0.10);
    }

    #[test]
    fn q4_security_surface_does_not_block_objective_under_freeze() {
        let patch = patch(
            "src/auth.py",
            "def normalize(value):\n    return value\n",
            "def normalize(value):\n    return value.strip()\n",
        );
        let context = context();
        let mut surfaces = infer_phase_b_surfaces(&patch, &context, &[0.99]).unwrap();
        surfaces
            .predicted_security_concerns
            .push(PredictedSecurityConcern {
                class: SecurityConcernClass::HardcodedSecret,
                chunk: ChunkId("src/auth.py#0".to_string()),
                line_range: (1, 2),
                cvss_estimate: Some(9.1),
                explanation: "pre-freeze Q4 display-only security row".to_string(),
            });
        let report =
            evaluate_mejepa_objective(MejepaObjective::default(), &patch, &surfaces, 0.99).unwrap();
        assert!(!report.pass_blocked);
        assert_eq!(report.cost.security_risk_score, 0.0);
        assert!(report.constraint_violations.iter().all(|violation| {
            violation.constraint != SafetyConstraintKind::NoSecretExposure
                && violation.constraint != SafetyConstraintKind::NoUnsafeCrypto
        }));
    }

    #[test]
    fn invalid_cost_weights_fail_closed() {
        let mut objective = MejepaObjective::default();
        objective.cost.oracle_fail = f32::NAN;
        let patch = patch(
            "src/a.py",
            "def a():\n    return 1\n",
            "def a():\n    return 2\n",
        );
        let context = context();
        let surfaces = infer_phase_b_surfaces(&patch, &context, &[0.99]).unwrap();
        let err = evaluate_mejepa_objective(objective, &patch, &surfaces, 0.99).unwrap_err();
        assert!(err.to_string().contains("cost weight must be finite"));
    }

    fn patch(path: &str, before: &str, after: &str) -> PatchBundle {
        PatchBundle::try_new(
            AstDiff {
                hunks: vec![DiffHunk {
                    path: path.into(),
                    pre_sha: sha256_bytes(before.as_bytes()),
                    post_sha: sha256_bytes(after.as_bytes()),
                    before: before.to_string(),
                    after: after.to_string(),
                }],
            },
            valid_witness_segment(),
            "TASK-FP-104 objective safety unit".to_string(),
            sha256_bytes(format!("{path}:{before}:{after}").as_bytes()),
        )
        .unwrap()
    }

    fn context() -> crate::TaskContext {
        crate::TaskContext {
            task_id: TaskId("task-fp-104-objective-safety-unit".to_string()),
            session_id: [4; 16],
            language: Language::Python,
            problem_statement: "TASK-FP-104 unit objective safety".to_string(),
            tests: vec![TestId("test_auth_required".to_string())],
            environment: TaskEnvironment {
                repo_root: ".".into(),
                python_version: Some("3.11".to_string()),
                os: std::env::consts::OS.to_string(),
            },
            claim_graph: None,
            skill_citations: Vec::new(),
        }
    }
}
