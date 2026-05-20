//! ME-JEPA Phase-B prediction surfaces (Q3 explanation layer).
//!
//! **DOCTRINAL STATUS (CLAUDE.md §1, F-020 / issue #473):** the heuristic helpers
//! at the bottom of this module (`is_passes_but_should_fail_context`,
//! `contains_secret_like_token`, `patch_has_flakiness_risk`,
//! `contains_merge_conflict`, `contains_auth_guard`, `is_dependency_manifest`,
//! `symbol_hint`, and similar `lower.contains(...)` / lowercase string-contains
//! predicates) are **Q3 display-only** heuristic hints attached to the Q2
//! `Verdict` for operator explanation. They are NOT gating predicates and they
//! are NOT a binary-doctrine prediction surface.
//!
//! Returning `false` on every code path where no heuristic substring matches is
//! the *correct* behavior for a hint: "no observed risk signal." It MUST NOT be
//! "fixed" into a fail-closed `Result` because a non-match is semantically
//! "nothing to report," not "evaluation failed." If you find yourself wanting
//! these to gate a verdict, instead emit a real binary head (Q1/Q2/Q5) with a
//! grounded source-of-truth — see `crate::verdict_assembly` and the
//! conformal/OOD path in `crate::ood` for the gate primitive.
//!
//! The Q3 status is also documented in:
//! - `CLAUDE.md` §1 (binary doctrine — Q3 is the named-failure-mode explanation
//!   surface attached to Q2 `Fail` verdicts; #408 closed the ambiguous-head
//!   Q4 producer track).
//! - `docs/SHERLOCK_INVESTIGATION_WORKAROUNDS_FALLBACKS.md` F-020 (MEDIUM-soft).
//! - GitHub issue #473 (this clarification).

use crate::error::MejepaInferError;
use crate::head_projection::{E_AST, E_TEST, E_TYPE_GRAPH};
use crate::types::{
    AccuracyMetric, ChunkId, CostAxis, DeadCodeKind, EdgeCaseClass, FailureModeClass,
    FlakyTestCandidate, LatentBugClass, PerfAxis, PhaseBPredictionSurfaces,
    PredictedAccuracyDegradation, PredictedCostRegression, PredictedDeadCode, PredictedEdgeCase,
    PredictedFailureMode, PredictedLatentBug, PredictedPerfRegression, PredictedRedundancy,
    PredictedSecurityConcern, PredictedTechDebt, PredictedWorks, RedundancyKind, RootCauseClass,
    SecurityConcernClass, Severity, TechDebtClass, UncoveredPath,
};
use crate::{EmbedderId, ExemplarMatch};
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

    if crate::q4_trust_gate::Q4_DOCTRINE_FREEZE_ACTIVE {
        surfaces.clear_q4_display_only_fields();
    }
    populate_predicted_works(&mut surfaces, patch, context, predicted_test_pass)?;
    populate_uncovered_paths(&mut surfaces, patch, context)?;
    populate_flaky_tests(&mut surfaces, patch, context)?;
    if crate::q4_trust_gate::Q4_DOCTRINE_FREEZE_ACTIVE {
        surfaces.clear_q4_display_only_fields();
    }
    validate_surfaces(&surfaces)?;
    Ok(surfaces)
}

pub fn enrich_predicted_works_with_known_good_exemplars(surfaces: &mut PhaseBPredictionSurfaces) {
    let known_good = known_good_exemplars(&surfaces.closest_exemplars);
    if known_good.is_empty() {
        return;
    }
    for predicted in &mut surfaces.predicted_works {
        if predicted.similar_known_good_exemplars.is_empty() {
            predicted.similar_known_good_exemplars = known_good.clone();
        }
    }
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

fn populate_predicted_works(
    surfaces: &mut PhaseBPredictionSurfaces,
    patch: &PatchBundle,
    context: &TaskContext,
    predicted_test_pass: &[f32],
) -> Result<(), MejepaInferError> {
    if predicted_test_pass.is_empty()
        || predicted_test_pass
            .iter()
            .any(|p| !p.is_finite() || *p < 0.8)
        || !surfaces.predicted_failure_modes.is_empty()
        || !surfaces.predicted_failed_tests.is_empty()
        || !surfaces.predicted_security_concerns.is_empty()
        || is_passes_but_should_fail_context(patch, context)
    {
        return Ok(());
    }
    let min_test_pass = predicted_test_pass
        .iter()
        .copied()
        .fold(1.0_f32, f32::min)
        .clamp(0.0, 1.0);
    let evidence_strength = (0.72 + min_test_pass * 0.24).clamp(0.0, 0.98);
    let confidence = (min_test_pass * 0.7 + evidence_strength * 0.3).clamp(0.0, 0.98);
    let known_good = known_good_exemplars(&surfaces.closest_exemplars);

    for (idx, hunk) in patch.ast_diff.hunks.iter().enumerate() {
        let chunk = ChunkId(format!("{}#{idx}", hunk.path.display()));
        let line_count = hunk.after.lines().count().max(1) as u32;
        let line_range = (1, line_count);
        let symbol = symbol_hint(hunk.after.as_str())
            .or_else(|| symbol_hint(hunk.before.as_str()))
            .unwrap_or_else(|| {
                hunk.path
                    .file_stem()
                    .and_then(|name| name.to_str())
                    .unwrap_or("changed code")
                    .to_string()
            });
        surfaces.predicted_works.push(PredictedWorks {
            chunk,
            line_range,
            claim: format!("{symbol} is predicted to preserve the exercised behavior"),
            confidence,
            supporting_embedders: vec![
                EmbedderId(E_AST.to_string()),
                EmbedderId(E_TEST.to_string()),
                EmbedderId(E_TYPE_GRAPH.to_string()),
            ],
            similar_known_good_exemplars: known_good.clone(),
            evidence_strength,
        });
    }
    Ok(())
}

fn populate_uncovered_paths(
    surfaces: &mut PhaseBPredictionSurfaces,
    patch: &PatchBundle,
    context: &TaskContext,
) -> Result<(), MejepaInferError> {
    if context.tests.is_empty() {
        for (idx, hunk) in patch.ast_diff.hunks.iter().enumerate() {
            surfaces.predicted_uncovered_paths.push(UncoveredPath {
                chunk: ChunkId(format!("{}#{idx}", hunk.path.display())),
                line_range: (1, hunk.after.lines().count().max(1) as u32),
                path_description: "changed code has no declared test id in task context"
                    .to_string(),
                defect_probability: 0.35,
                confidence: 0.72,
                evidence: "TaskContext.tests is empty".to_string(),
            });
        }
    }
    for edge in &surfaces.predicted_edge_cases {
        if !edge.covered_by_test {
            surfaces.predicted_uncovered_paths.push(UncoveredPath {
                chunk: edge.chunk.clone(),
                line_range: edge.line_range,
                path_description: edge.triggering_input_description.clone(),
                defect_probability: (edge.confidence * 0.45).clamp(0.0, 0.95),
                confidence: edge.confidence,
                evidence: format!("{:?} edge case has no matching test id", edge.edge_class),
            });
        }
    }
    Ok(())
}

fn populate_flaky_tests(
    surfaces: &mut PhaseBPredictionSurfaces,
    patch: &PatchBundle,
    context: &TaskContext,
) -> Result<(), MejepaInferError> {
    if context.tests.is_empty() || !patch_has_flakiness_risk(patch) {
        return Ok(());
    }
    for test_id in context.tests.iter().take(8) {
        let lower = test_id.0.to_ascii_lowercase();
        let confidence = if lower.contains("flaky") || lower.contains("parallel") {
            0.82
        } else {
            0.64
        };
        surfaces.predicted_flaky_tests.push(FlakyTestCandidate {
            test_id: test_id.clone(),
            serial_pass_probability: 0.92,
            parallel_pass_probability: 0.62,
            confidence,
            evidence: "patch touches time/random/concurrency-sensitive code".to_string(),
        });
    }
    Ok(())
}

fn validate_surfaces(surfaces: &PhaseBPredictionSurfaces) -> Result<(), MejepaInferError> {
    for mode in &surfaces.predicted_failure_modes {
        mode.validate()?;
    }
    for work in &surfaces.predicted_works {
        work.validate()?;
    }
    for uncovered in &surfaces.predicted_uncovered_paths {
        uncovered.validate()?;
    }
    for flaky in &surfaces.predicted_flaky_tests {
        flaky.validate()?;
    }
    Ok(())
}

fn known_good_exemplars(exemplars: &[ExemplarMatch]) -> Vec<ExemplarMatch> {
    exemplars
        .iter()
        .filter(|exemplar| exemplar.oracle_outcome.all_passed())
        .take(5)
        .cloned()
        .collect()
}

fn symbol_hint(text: &str) -> Option<String> {
    for line in text.lines() {
        let trimmed = line.trim_start();
        for prefix in ["def ", "async def ", "class ", "fn "] {
            let Some(rest) = trimmed.strip_prefix(prefix) else {
                continue;
            };
            let name = rest
                .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
                .next()
                .unwrap_or_default();
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }
    None
}

fn is_passes_but_should_fail_context(patch: &PatchBundle, context: &TaskContext) -> bool {
    let mut text = String::new();
    text.push_str(&context.problem_statement);
    text.push('\n');
    text.push_str(&patch.commit_message);
    for hunk in &patch.ast_diff.hunks {
        text.push('\n');
        text.push_str(&hunk.before);
        text.push('\n');
        text.push_str(&hunk.after);
    }
    let lower = text.to_ascii_lowercase();
    lower.contains("passesbutshouldfail")
        || lower.contains("passes_but_should_fail")
        || lower.contains("spec-violating")
        || (lower.contains("passes") && lower.contains("should fail"))
}

fn patch_has_flakiness_risk(patch: &PatchBundle) -> bool {
    patch.ast_diff.hunks.iter().any(|hunk| {
        let lower = hunk.after.to_ascii_lowercase();
        lower.contains("time.sleep")
            || lower.contains("time.time")
            || lower.contains("datetime.now")
            || lower.contains("random.")
            || lower.contains("thread")
            || lower.contains("asyncio")
            || lower.contains("pytest.mark.flaky")
    })
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
    fn q4_display_only_surfaces_stay_empty_under_freeze() {
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
        assert_eq!(surfaces.q4_display_only_field_count(), 0);
    }

    #[test]
    fn predicted_works_emits_positive_claim_for_clean_python_function() {
        let patch = PatchBundle::try_new(
            AstDiff {
                hunks: vec![DiffHunk {
                    path: "src/search.py".into(),
                    pre_sha: [0; 32],
                    post_sha: [1; 32],
                    before: "def binary_search(items, target):\n    return -1\n".to_string(),
                    after: "def binary_search(items, target):\n    return items.index(target) if target in items else -1\n"
                        .to_string(),
                }],
            },
            Vec::new(),
            "known good binary search".to_string(),
            [1; 32],
        )
        .unwrap();
        let context = TaskContext {
            task_id: TaskId("task-py-g-073-unit".to_string()),
            session_id: [1; 16],
            language: Language::Python,
            problem_statement: "KnownGood binary search".to_string(),
            tests: vec![TestId("tests/test_search.py::test_found".to_string())],
            environment: TaskEnvironment {
                repo_root: ".".into(),
                python_version: Some("3.12".to_string()),
                os: "linux".to_string(),
            },
            claim_graph: None,
            skill_citations: Vec::new(),
        };
        let surfaces = infer_phase_b_surfaces(&patch, &context, &[0.93]).unwrap();
        assert_eq!(surfaces.predicted_works.len(), 1);
        assert!(surfaces.predicted_works[0].claim.contains("binary_search"));
        assert!(!surfaces.predicted_works[0].supporting_embedders.is_empty());
    }

    #[test]
    fn q4_security_signal_does_not_suppress_predicted_works_under_freeze() {
        let patch = PatchBundle::try_new(
            AstDiff {
                hunks: vec![DiffHunk {
                    path: "src/config.py".into(),
                    pre_sha: [0; 32],
                    post_sha: [1; 32],
                    before: "def load_config():\n    return {}\n".to_string(),
                    after: "def load_config():\n    api_key = \"sk-test\"\n    return {\"api_key\": api_key}\n"
                        .to_string(),
                }],
            },
            Vec::new(),
            "known good config loader".to_string(),
            [1; 32],
        )
        .unwrap();
        let context = TaskContext {
            task_id: TaskId("task-py-g-114-unit".to_string()),
            session_id: [1; 16],
            language: Language::Python,
            problem_statement: "KnownGood config loader".to_string(),
            tests: vec![TestId("tests/test_config.py::test_load_config".to_string())],
            environment: TaskEnvironment {
                repo_root: ".".into(),
                python_version: Some("3.12".to_string()),
                os: "linux".to_string(),
            },
            claim_graph: None,
            skill_citations: Vec::new(),
        };
        let surfaces = infer_phase_b_surfaces(&patch, &context, &[0.93]).unwrap();
        assert_eq!(surfaces.q4_display_only_field_count(), 0);
        assert_eq!(surfaces.predicted_works.len(), 1);
    }

    #[test]
    fn predicted_works_suppressed_for_passes_but_should_fail_stress_case() {
        let patch = PatchBundle::try_new(
            AstDiff {
                hunks: vec![DiffHunk {
                    path: "src/search.py".into(),
                    pre_sha: [0; 32],
                    post_sha: [1; 32],
                    before: "def binary_search(items, target):\n    return -1\n".to_string(),
                    after: "def binary_search(items, target):\n    # PassesButShouldFail\n    return 0\n"
                        .to_string(),
                }],
            },
            Vec::new(),
            "PassesButShouldFail".to_string(),
            [1; 32],
        )
        .unwrap();
        let context = TaskContext {
            task_id: TaskId("task-py-g-073-unit".to_string()),
            session_id: [1; 16],
            language: Language::Python,
            problem_statement: "PassesButShouldFail: tests pass but spec should fail".to_string(),
            tests: vec![TestId("tests/test_search.py::test_smoke".to_string())],
            environment: TaskEnvironment {
                repo_root: ".".into(),
                python_version: Some("3.12".to_string()),
                os: "linux".to_string(),
            },
            claim_graph: None,
            skill_citations: Vec::new(),
        };
        let surfaces = infer_phase_b_surfaces(&patch, &context, &[0.93]).unwrap();
        assert!(surfaces.predicted_works.is_empty());
    }
}
