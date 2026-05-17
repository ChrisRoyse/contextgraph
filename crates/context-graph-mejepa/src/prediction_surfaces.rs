use crate::error::MejepaInferError;
use crate::types::{
    AccuracyMetric, ChunkId, CostAxis, DeadCodeKind, EdgeCaseClass, FailureModeClass,
    LatentBugClass, PerfAxis, PhaseBPredictionSurfaces, PredictedAccuracyDegradation,
    PredictedCostRegression, PredictedDeadCode, PredictedEdgeCase, PredictedFailureMode,
    PredictedLatentBug, PredictedPerfRegression, PredictedRedundancy, PredictedSecurityConcern,
    PredictedTechDebt, RedundancyKind, RootCauseClass, SecurityConcernClass, Severity,
    TechDebtClass,
};
use crate::{PatchBundle, TaskContext};

pub fn covered_chunks_for_patch(patch: &PatchBundle) -> Result<Vec<ChunkId>, MejepaInferError> {
    let mut out = Vec::with_capacity(patch.ast_diff.hunks.len());
    for (idx, hunk) in patch.ast_diff.hunks.iter().enumerate() {
        out.push(ChunkId(format!("{}#{idx}", hunk.path.display())));
    }
    for chunk in &out {
        chunk.validate("covered_chunks")?;
    }
    Ok(out)
}

pub fn infer_phase_b_surfaces(
    patch: &PatchBundle,
    context: &TaskContext,
    predicted_test_pass: &[f32],
) -> Result<PhaseBPredictionSurfaces, MejepaInferError> {
    let covered = covered_chunks_for_patch(patch)?;
    let mut surfaces = PhaseBPredictionSurfaces::default();
    for (idx, hunk) in patch.ast_diff.hunks.iter().enumerate() {
        let chunk = covered[idx].clone();
        let after_lower = hunk.after.to_ascii_lowercase();
        let after_lines = hunk.after.lines().count().max(1) as u32;
        let line_range = (1, after_lines);
        if contains_merge_conflict(&hunk.after) {
            surfaces.predicted_failure_modes.push(failure(
                FailureModeClass::SyntaxError,
                chunk.clone(),
                line_range,
                0.98,
                Severity::Critical,
                "merge conflict markers make the file syntactically invalid",
                RootCauseClass::InterfaceError,
            ));
        }
        if predicted_test_pass.iter().any(|p| *p < 0.5) {
            surfaces.predicted_failure_modes.push(failure(
                FailureModeClass::AssertionMismatch,
                chunk.clone(),
                line_range,
                1.0 - predicted_test_pass.iter().copied().fold(1.0_f32, f32::min),
                Severity::High,
                "oracle head predicts at least one test below the pass threshold",
                RootCauseClass::LogicError,
            ));
        }
        if after_lower.contains("unwrap()")
            || after_lower.contains(" none")
            || after_lower.contains(" null")
            || after_lower.contains("undefined")
        {
            surfaces.predicted_edge_cases.push(PredictedEdgeCase {
                edge_class: EdgeCaseClass::OptionalNotPresent,
                chunk: chunk.clone(),
                line_range,
                triggering_input_description: "missing/None/null value reaches this path"
                    .to_string(),
                covered_by_test: context.tests.iter().any(|test| {
                    let id = test.0.to_ascii_lowercase();
                    id.contains("none") || id.contains("null") || id.contains("missing")
                }),
                confidence: 0.72,
            });
        }
        if after_lower.contains("len() == 0")
            || after_lower.contains(".is_empty()")
            || after_lower.contains("empty")
        {
            surfaces.predicted_edge_cases.push(PredictedEdgeCase {
                edge_class: EdgeCaseClass::EmptyInput,
                chunk: chunk.clone(),
                line_range,
                triggering_input_description: "empty collection/string input".to_string(),
                covered_by_test: context
                    .tests
                    .iter()
                    .any(|test| test.0.to_ascii_lowercase().contains("empty")),
                confidence: 0.76,
            });
        }
        if after_lower.contains("todo")
            || after_lower.contains("fixme")
            || after_lower.contains("hack")
        {
            surfaces.predicted_tech_debt_added.push(PredictedTechDebt {
                debt_class: TechDebtClass::TodoWithoutOwner,
                chunk: chunk.clone(),
                line_range,
                severity: Severity::Medium,
                explanation: "patch introduces an ownerless TODO/FIXME/HACK marker".to_string(),
            });
        }
        if hunk.after.lines().count() > 80 {
            surfaces.predicted_tech_debt_added.push(PredictedTechDebt {
                debt_class: TechDebtClass::LongFunction,
                chunk: chunk.clone(),
                line_range,
                severity: Severity::Medium,
                explanation: "single hunk is long enough to hide branch and review risk"
                    .to_string(),
            });
        }
        if after_lower.contains("except:")
            || after_lower.contains("catch (")
            || after_lower.contains("catch(")
        {
            surfaces.predicted_latent_bugs.push(PredictedLatentBug {
                bug_class: LatentBugClass::InconsistentErrorHandling,
                chunk: chunk.clone(),
                line_range,
                confidence: 0.68,
                severity: Severity::Medium,
                explanation: "broad exception handling may hide real failures".to_string(),
            });
        }
        if after_lower.contains("unused")
            || after_lower.contains("#[allow(dead_code)]")
            || after_lower.contains("unreachable!")
        {
            surfaces.predicted_dead_code.push(PredictedDeadCode {
                kind: DeadCodeKind::UnusedFunction,
                chunk: chunk.clone(),
                line_range,
                reason: "static signal references unused/dead code".to_string(),
            });
        }
        if after_lower.matches("for ").count() >= 2 || after_lower.contains(".clone()") {
            surfaces
                .predicted_perf_regressions
                .push(PredictedPerfRegression {
                    axis: PerfAxis::CpuMs,
                    chunk: chunk.clone(),
                    predicted_delta_pct: 10.0,
                    baseline_value: None,
                    confidence: 0.61,
                    explanation:
                        "nested iteration or clone-heavy path suggests CPU/allocation growth"
                            .to_string(),
                });
        }
        if contains_secret_like_token(&hunk.after) {
            surfaces
                .predicted_security_concerns
                .push(PredictedSecurityConcern {
                    class: SecurityConcernClass::HardcodedSecret,
                    chunk: chunk.clone(),
                    line_range,
                    cvss_estimate: Some(7.5),
                    explanation:
                        "patch text contains a token/key pattern that must be redacted before embedding"
                            .to_string(),
                });
        }
        if after_lower.contains("shell=true")
            || after_lower.contains("os.system(")
            || after_lower.contains("subprocess.call(")
            || after_lower.contains("subprocess.run(")
            || after_lower.contains("subprocess.popen(")
        {
            surfaces
                .predicted_security_concerns
                .push(PredictedSecurityConcern {
                    class: SecurityConcernClass::CommandInjection,
                    chunk: chunk.clone(),
                    line_range,
                    cvss_estimate: Some(8.1),
                    explanation:
                        "patch executes shell commands; Q4 security SAST labels train this as command-injection risk"
                            .to_string(),
                });
        }
        let before_lower = hunk.before.to_ascii_lowercase();
        let before_has_auth = contains_auth_guard(&before_lower);
        if before_has_auth && !contains_auth_guard(&after_lower) {
            surfaces
                .predicted_security_concerns
                .push(PredictedSecurityConcern {
                    class: SecurityConcernClass::MissingAuth,
                    chunk: chunk.clone(),
                    line_range,
                    cvss_estimate: Some(8.0),
                    explanation: "patch removes an authentication/authorization guard".to_string(),
                });
        }
        if after_lower.contains("md5") || after_lower.contains("sha1") {
            surfaces
                .predicted_security_concerns
                .push(PredictedSecurityConcern {
                    class: SecurityConcernClass::InsecureCryptoAlgo,
                    chunk: chunk.clone(),
                    line_range,
                    cvss_estimate: Some(5.9),
                    explanation: "patch references a weak cryptographic digest".to_string(),
                });
        }
        if after_lower.contains("f1")
            || after_lower.contains("accuracy")
            || after_lower.contains("auc")
        {
            surfaces
                .predicted_accuracy_degradations
                .push(PredictedAccuracyDegradation {
                    metric: AccuracyMetric::Accuracy,
                    chunk: chunk.clone(),
                    predicted_delta: -0.02,
                    baseline_value: None,
                    confidence: 0.55,
                    explanation: "patch touches an ML/data quality metric path".to_string(),
                });
        }
        if is_dependency_manifest(&hunk.path) {
            surfaces
                .predicted_cost_regressions
                .push(PredictedCostRegression {
                    axis: CostAxis::DependencyCount,
                    chunk: chunk.clone(),
                    predicted_delta: 1.0,
                    baseline_value: None,
                    explanation:
                        "dependency manifest changed, increasing build/supply-chain surface"
                            .to_string(),
                });
        }
        if after_lower.contains("duplicated") || after_lower.contains("copy paste") {
            surfaces.predicted_redundant_code.push(PredictedRedundancy {
                kind: RedundancyKind::DuplicatedFunction,
                chunk,
                also_at: Vec::new(),
                similarity: 0.7,
                explanation: "patch text explicitly signals duplicated implementation".to_string(),
            });
        }
    }
    for mode in &surfaces.predicted_failure_modes {
        mode.validate()?;
    }
    Ok(surfaces)
}

fn failure(
    failure_class: FailureModeClass,
    chunk: ChunkId,
    line_range: (u32, u32),
    confidence: f32,
    severity: Severity,
    explanation: &str,
    root_cause_class: RootCauseClass,
) -> PredictedFailureMode {
    PredictedFailureMode {
        failure_class,
        chunk,
        line_range,
        confidence: confidence.clamp(0.0, 1.0),
        severity,
        explanation: explanation.to_string(),
        contributing_embedders: Vec::new(),
        root_cause_class,
    }
}

fn contains_merge_conflict(text: &str) -> bool {
    text.contains("<<<<<<<") || text.contains("=======") || text.contains(">>>>>>>")
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

fn contains_auth_guard(lower: &str) -> bool {
    lower.contains("require_auth")
        || lower.contains("is_authenticated")
        || lower.contains("has_permission")
        || lower.contains("check_permission")
        || lower.contains("authorize(")
        || lower.contains("authenticated")
}

fn is_dependency_manifest(path: &std::path::Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| {
            matches!(
                name,
                "Cargo.toml"
                    | "Cargo.lock"
                    | "package.json"
                    | "package-lock.json"
                    | "pyproject.toml"
                    | "requirements.txt"
                    | "go.mod"
                    | "pom.xml"
            )
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{AstDiff, DiffHunk, Language, PatchBundle, TaskEnvironment, TaskId, TestId};

    #[test]
    fn detects_hardcoded_secret_and_empty_edge() {
        let patch = PatchBundle::try_new(
            AstDiff {
                hunks: vec![DiffHunk {
                    path: "src/lib.rs".into(),
                    pre_sha: [0; 32],
                    post_sha: [1; 32],
                    before: String::new(),
                    after: "let api_key = \"sk-test\";\nif values.is_empty() { return; }"
                        .to_string(),
                }],
            },
            Vec::new(),
            "test".to_string(),
            [1; 32],
        )
        .unwrap();
        let context = TaskContext {
            task_id: TaskId("task".to_string()),
            session_id: [1; 16],
            language: Language::Rust,
            problem_statement: "test".to_string(),
            tests: vec![TestId("test_empty".to_string())],
            environment: TaskEnvironment {
                repo_root: ".".into(),
                python_version: None,
                os: "linux".to_string(),
            },
            claim_graph: None,
            skill_citations: Vec::new(),
        };
        let surfaces = infer_phase_b_surfaces(&patch, &context, &[0.9]).unwrap();
        assert_eq!(surfaces.predicted_security_concerns.len(), 1);
        assert_eq!(surfaces.predicted_edge_cases.len(), 1);
    }
}
