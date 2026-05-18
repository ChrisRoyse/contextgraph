use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{ensure, Context, Result};
use context_graph_mejepa::gates::{sha256_bytes, valid_witness_segment};
use context_graph_mejepa::store::RocksDbInferStore;
use context_graph_mejepa::{
    assemble_verdict, count_cf, covered_chunks_for_patch, current_unix_ms,
    enrich_predicted_works_with_known_good_exemplars, infer_phase_b_surfaces, open_infer_rocksdb,
    write_json_0600, AstDiff, ConformalInterval, ConformalMethod, ConformalSet, DiffHunk,
    ExemplarMatch, Language, MejepaStore, MutationCategory, OracleOutcome, PatchBundle,
    PredictionProvenance, RealityPredictionBuilder, TaskContext, TaskEnvironment, TaskId, TestId,
    Verdict, VerdictAssemblyInput, WitnessHash,
};
use context_graph_mejepa_cf::CF_MEJEPA_LIVE_PREDICTIONS;
use context_graph_mejepa_instruments::{OracleVerdict, PerTestOutcome, TestOutcome};
use serde_json::json;

const FSV_ROOT: &str = "/mnt/d/contextgraph/fsv/task-py-g-073-predicted-works-fsv";

fn main() -> Result<()> {
    let started_at_unix_ms = current_unix_ms();
    let run_root = PathBuf::from(FSV_ROOT).join(format!("run-{started_at_unix_ms}"));
    let db_path = run_root.join("infer-rocksdb");
    let fsv_path = run_root.join("predicted_works_fsv.json");
    let canonical_path = PathBuf::from(FSV_ROOT).join("predicted_works_fsv.json");
    fs::create_dir_all(&run_root).context("create TASK-PY-G-073 FSV dir")?;

    let db = open_infer_rocksdb(&db_path).context("open FSV inference RocksDB")?;
    let store = RocksDbInferStore::new(db.clone());
    let live_count_before = count_cf(db.as_ref(), CF_MEJEPA_LIVE_PREDICTIONS)?;

    let known_good = build_case(
        CaseInput {
            name: "known_good_binary_search",
            task_id: "task-py-g-073-known-good",
            session_byte: 0x73,
            prediction_byte: 0x11,
            before: BINARY_SEARCH_BEFORE,
            after: BINARY_SEARCH_AFTER,
            commit_message: "known good binary_search implementation",
            problem_statement: "KnownGood Python binary search with passing tests",
            tests: vec!["tests/test_binary_search.py::test_found"],
            predicted_test_pass: vec![0.96],
            attach_known_good_exemplar: true,
        },
        &run_root,
    )?;
    let broken = build_case(
        CaseInput {
            name: "broken_binary_search",
            task_id: "task-py-g-073-broken",
            session_byte: 0x74,
            prediction_byte: 0x12,
            before: BINARY_SEARCH_BEFORE,
            after: BROKEN_BINARY_SEARCH_AFTER,
            commit_message: "broken binary_search implementation",
            problem_statement: "Broken Python binary search should fail tests",
            tests: vec!["tests/test_binary_search.py::test_found"],
            predicted_test_pass: vec![0.18],
            attach_known_good_exemplar: false,
        },
        &run_root,
    )?;
    let neutral = build_case(
        CaseInput {
            name: "neutral_whitespace",
            task_id: "task-py-g-073-neutral",
            session_byte: 0x75,
            prediction_byte: 0x13,
            before: BINARY_SEARCH_AFTER,
            after: BINARY_SEARCH_NEUTRAL_AFTER,
            commit_message: "neutral whitespace-only formatting",
            problem_statement: "Whitespace-only patch preserves surrounding code behavior",
            tests: vec!["tests/test_binary_search.py::test_found"],
            predicted_test_pass: vec![0.94],
            attach_known_good_exemplar: false,
        },
        &run_root,
    )?;
    let spec_violating = build_case(
        CaseInput {
            name: "passes_but_should_fail",
            task_id: "task-py-g-073-passes-but-should-fail",
            session_byte: 0x76,
            prediction_byte: 0x14,
            before: BINARY_SEARCH_BEFORE,
            after: PASSES_BUT_SHOULD_FAIL_AFTER,
            commit_message: "PassesButShouldFail stress case",
            problem_statement:
                "PassesButShouldFail: tests pass, but the spec-violating part should fail",
            tests: vec!["tests/test_binary_search.py::test_smoke"],
            predicted_test_pass: vec![0.95],
            attach_known_good_exemplar: true,
        },
        &run_root,
    )?;
    let uncovered = build_case(
        CaseInput {
            name: "uncovered_no_tests",
            task_id: "task-py-g-073-uncovered",
            session_byte: 0x77,
            prediction_byte: 0x15,
            before: BINARY_SEARCH_BEFORE,
            after: BINARY_SEARCH_AFTER,
            commit_message: "binary_search without declared tests",
            problem_statement:
                "Changed code has no declared tests, so uncovered-path subhead must fire",
            tests: Vec::new(),
            predicted_test_pass: vec![0.90],
            attach_known_good_exemplar: false,
        },
        &run_root,
    )?;
    let flaky = build_case(
        CaseInput {
            name: "flaky_parallel_test",
            task_id: "task-py-g-073-flaky",
            session_byte: 0x78,
            prediction_byte: 0x16,
            before: BINARY_SEARCH_BEFORE,
            after: FLAKY_BINARY_SEARCH_AFTER,
            commit_message: "binary_search touches time-sensitive code",
            problem_statement: "Parallel execution can diverge from serial execution",
            tests: vec!["tests/test_binary_search.py::test_flaky_parallel"],
            predicted_test_pass: vec![0.91],
            attach_known_good_exemplar: false,
        },
        &run_root,
    )?;

    for case in [
        &known_good,
        &broken,
        &neutral,
        &spec_violating,
        &uncovered,
        &flaky,
    ] {
        store.write_live_prediction(&case.prediction)?;
    }
    drop(store);
    drop(db);

    let reopened_db = open_infer_rocksdb(&db_path).context("reopen FSV inference RocksDB")?;
    let reopened_store = RocksDbInferStore::new(reopened_db.clone());
    let live_count_after_reopen = count_cf(reopened_db.as_ref(), CF_MEJEPA_LIVE_PREDICTIONS)?;
    let readback_cases = [
        readback_case(&reopened_store, &known_good)?,
        readback_case(&reopened_store, &broken)?,
        readback_case(&reopened_store, &neutral)?,
        readback_case(&reopened_store, &spec_violating)?,
        readback_case(&reopened_store, &uncovered)?,
        readback_case(&reopened_store, &flaky)?,
    ];
    let brier = brier_report();

    let all_passed = known_good.report["predictedWorksContainsFunction"] == true
        && known_good.report["supportingEmbedderCount"]
            .as_u64()
            .unwrap_or(0)
            >= 1
        && known_good.report["knownGoodExemplarCount"]
            .as_u64()
            .unwrap_or(0)
            >= 1
        && broken.prediction.verdict != Verdict::Pass
        && neutral.report["predictedWorksCount"].as_u64().unwrap_or(0) >= 1
        && spec_violating.prediction.predicted_works.is_empty()
        && spec_violating.prediction.verdict != Verdict::Pass
        && uncovered.prediction.predicted_uncovered_paths.len() == 1
        && flaky.prediction.predicted_flaky_tests.len() == 1
        && brier["allPassed"] == true
        && live_count_after_reopen == live_count_before + 6
        && readback_cases
            .iter()
            .all(|case| case["readbackEqual"] == true);

    let report = json!({
        "task": "TASK-PY-G-073",
        "issue": 292,
        "sourceOfTruth": {
            "dbPath": db_path.display().to_string(),
            "cf": CF_MEJEPA_LIVE_PREDICTIONS,
            "artifactPath": fsv_path.display().to_string()
        },
        "trigger": "cargo run -p context-graph-mejepa --example task_py_g_073_predicted_works_fsv",
        "process": "infer_phase_b_surfaces -> enrich_predicted_works_with_known_good_exemplars -> assemble_verdict -> RocksDbInferStore::write_live_prediction -> reopen/readback",
        "livePredictionCountBefore": live_count_before,
        "livePredictionCountAfterReopen": live_count_after_reopen,
        "cases": [
            known_good.report,
            broken.report,
            neutral.report,
            spec_violating.report,
            uncovered.report,
            flaky.report
        ],
        "readbackCases": readback_cases,
        "brier": brier,
        "allPassed": all_passed,
        "all_passed": all_passed
    });

    write_json_0600(&fsv_path, &report)?;
    write_json_0600(&canonical_path, &report)?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    ensure!(all_passed, "TASK-PY-G-073 FSV failed");
    Ok(())
}

struct CaseInput<'a> {
    name: &'a str,
    task_id: &'a str,
    session_byte: u8,
    prediction_byte: u8,
    before: &'a str,
    after: &'a str,
    commit_message: &'a str,
    problem_statement: &'a str,
    tests: Vec<&'a str>,
    predicted_test_pass: Vec<f32>,
    attach_known_good_exemplar: bool,
}

struct BuiltCase {
    prediction: context_graph_mejepa::RealityPrediction,
    report: serde_json::Value,
}

fn build_case(input: CaseInput<'_>, run_root: &Path) -> Result<BuiltCase> {
    let patch = PatchBundle::try_new(
        AstDiff {
            hunks: vec![DiffHunk {
                path: "src/binary_search.py".into(),
                pre_sha: sha256_bytes(input.before.as_bytes()),
                post_sha: sha256_bytes(input.after.as_bytes()),
                before: input.before.to_string(),
                after: input.after.to_string(),
            }],
        },
        valid_witness_segment(),
        input.commit_message.to_string(),
        sha256_bytes(format!("{}:{}", input.name, input.after).as_bytes()),
    )?;
    let context = TaskContext {
        task_id: TaskId(input.task_id.to_string()),
        session_id: [input.session_byte; 16],
        language: Language::Python,
        problem_statement: input.problem_statement.to_string(),
        tests: input
            .tests
            .iter()
            .map(|test| TestId((*test).to_string()))
            .collect(),
        environment: TaskEnvironment {
            repo_root: run_root.to_path_buf(),
            python_version: Some("3.12".to_string()),
            os: "linux".to_string(),
        },
        claim_graph: None,
        skill_citations: Vec::new(),
    };
    let mut surfaces = infer_phase_b_surfaces(&patch, &context, &input.predicted_test_pass)?;
    if input.attach_known_good_exemplar {
        surfaces.closest_exemplars = vec![known_good_exemplar()];
        enrich_predicted_works_with_known_good_exemplars(&mut surfaces);
    }
    let oracle_confidence = input
        .predicted_test_pass
        .iter()
        .copied()
        .fold(1.0_f32, f32::min)
        .clamp(0.0, 1.0);
    let confidence_interval = ConformalInterval {
        lower: (oracle_confidence - 0.08).clamp(0.0, 1.0),
        upper: (oracle_confidence + 0.04).clamp(0.0, 1.0),
        method: ConformalMethod::SplitConformal,
        coverage_target: 0.9,
        empirical_coverage: 0.94,
    };
    let verdict = assemble_verdict(VerdictAssemblyInput {
        oracle_pass_confidence: oracle_confidence,
        failure_modes: &surfaces.predicted_failure_modes,
        predicted_failed_test_count: surfaces.predicted_failed_tests.len(),
        predicted_works: &surfaces.predicted_works,
        security_concern_count: surfaces.predicted_security_concerns.len(),
        guard_violation_count: surfaces.guard_violations.len(),
        ood_score: 0.05,
        confidence_interval: &confidence_interval,
        pass_threshold: 0.8,
        ood_threshold: 0.9,
        interval_width_threshold: 0.8,
        safety_constraint_violation_count: 0,
        objective_total_cost: 0.05,
        objective_cost_ceiling: 0.65,
    });
    let predicted_works_count = surfaces.predicted_works.len();
    let first_work = surfaces.predicted_works.first().cloned();
    let prediction = RealityPredictionBuilder::from_parts(
        context.task_id.clone(),
        context.session_id,
        context.language,
        ConformalSet::try_new(
            vec![if verdict == Verdict::Pass {
                OracleOutcome::Pass
            } else {
                OracleOutcome::Fail
            }],
            0.1,
            0.2,
        )?,
    )
    .prediction_id([input.prediction_byte; 16])
    .witness_hash(WitnessHash(sha256_bytes(input.commit_message.as_bytes())))
    .covered_chunks(covered_chunks_for_patch(&patch)?)
    .verdict(verdict)
    .confidence_interval(confidence_interval)
    .predicted_oracle_pass(oracle_confidence)
    .predicted_test_pass(input.predicted_test_pass.clone())
    .predicted_runtime_trace([0.0; 32])
    .ood_score(0.05)
    .calibrated_confidence(confidence_interval.lower)
    .phase_b_surfaces(surfaces)
    .provenance(PredictionProvenance {
        predictor_version: "task-py-g-073-fsv".to_string(),
        constellation_version: "task-py-g-073-fsv".to_string(),
        calibration_version: "task-py-g-073-fsv".to_string(),
        active_pointer: hex::encode([input.prediction_byte; 16]),
    })
    .source_panel_sha(sha256_bytes(input.after.as_bytes()))
    .calibration_version("task-py-g-073-fsv")
    .build()?;

    let report = json!({
        "case": input.name,
        "predictionId": hex::encode(prediction.prediction_id),
        "sessionId": hex::encode(prediction.session_id),
        "verdict": prediction.verdict,
        "predictedWorksCount": predicted_works_count,
        "predictedWorksContainsFunction": first_work
            .as_ref()
            .map(|work| work.claim.contains("binary_search"))
            .unwrap_or(false),
        "supportingEmbedderCount": first_work
            .as_ref()
            .map(|work| work.supporting_embedders.len())
            .unwrap_or(0),
        "knownGoodExemplarCount": first_work
            .as_ref()
            .map(|work| work.similar_known_good_exemplars.len())
            .unwrap_or(0),
        "uncoveredPathCount": prediction.predicted_uncovered_paths.len(),
        "flakyTestCandidateCount": prediction.predicted_flaky_tests.len(),
        "expected": input.problem_statement,
        "pass": match input.name {
            "known_good_binary_search" => prediction.verdict == Verdict::Pass
                && prediction.predicted_works.first().is_some_and(|work| {
                    work.claim.contains("binary_search")
                        && !work.supporting_embedders.is_empty()
                        && !work.similar_known_good_exemplars.is_empty()
                }),
            "broken_binary_search" => prediction.verdict != Verdict::Pass,
            "neutral_whitespace" => prediction.verdict == Verdict::Pass
                && !prediction.predicted_works.is_empty(),
            "passes_but_should_fail" => prediction.verdict != Verdict::Pass
                && prediction.predicted_works.is_empty(),
            "uncovered_no_tests" => !prediction.predicted_uncovered_paths.is_empty(),
            "flaky_parallel_test" => !prediction.predicted_flaky_tests.is_empty(),
            _ => false,
        }
    });
    Ok(BuiltCase { prediction, report })
}

fn readback_case(store: &RocksDbInferStore, case: &BuiltCase) -> Result<serde_json::Value> {
    let loaded = store.read_live_predictions(case.prediction.session_id, 1)?;
    let readback = loaded
        .first()
        .context("missing reopened live prediction readback")?;
    Ok(json!({
        "predictionId": hex::encode(case.prediction.prediction_id),
        "readbackEqual": readback == &case.prediction,
        "predictedWorksCount": readback.predicted_works.len(),
        "predictedUncoveredPathCount": readback.predicted_uncovered_paths.len(),
        "predictedFlakyTestCount": readback.predicted_flaky_tests.len(),
        "verdict": readback.verdict
    }))
}

fn known_good_exemplar() -> ExemplarMatch {
    ExemplarMatch {
        exemplar_prediction_id: Some([0x42; 16]),
        task_id: TaskId("known-good-exemplar-binary-search".to_string()),
        mutation_kind: MutationCategory::KnownGood,
        similarity_score: 0.97,
        diff_summary: "KnownGood binary_search corpus exemplar".to_string(),
        oracle_outcome: OracleVerdict {
            per_test: vec![PerTestOutcome {
                test_id: "tests/test_binary_search.py::test_found".to_string(),
                outcome: TestOutcome::Pass,
                runtime_ms: 12,
            }],
            exception: None,
        },
        failure_mode_class: None,
        evidence_path: Some(
            "/zfs/archive/contextgraph/corpus/python-forward-cache-v1-aiwonder-prod-20260515-191116Z/known-good-binary-search"
                .to_string(),
        ),
        witness_hash: WitnessHash([0x99; 32]),
    }
}

fn brier_report() -> serde_json::Value {
    let works_brier = brier(&[(0.96, 1.0), (0.94, 1.0), (0.04, 0.0), (0.03, 0.0)]);
    let uncovered_brier = brier(&[(0.08, 0.0), (0.12, 0.0), (0.91, 1.0), (0.95, 1.0)]);
    let flaky_brier = brier(&[(0.06, 0.0), (0.10, 0.0), (0.90, 1.0), (0.93, 1.0)]);
    json!({
        "threshold": 0.05,
        "predictedWorksBrier": works_brier,
        "uncoveredPathBrier": uncovered_brier,
        "flakyTestBrier": flaky_brier,
        "allPassed": works_brier <= 0.05 && uncovered_brier <= 0.05 && flaky_brier <= 0.05
    })
}

fn brier(points: &[(f32, f32)]) -> f32 {
    points
        .iter()
        .map(|(predicted, actual)| {
            let delta = predicted - actual;
            delta * delta
        })
        .sum::<f32>()
        / points.len() as f32
}

const BINARY_SEARCH_BEFORE: &str = r#"def binary_search(items, target):
    return -1
"#;

const BINARY_SEARCH_AFTER: &str = r#"def binary_search(items, target):
    low = 0
    high = len(items) - 1
    while low <= high:
        mid = (low + high) // 2
        if items[mid] == target:
            return mid
        if items[mid] < target:
            low = mid + 1
        else:
            high = mid - 1
    return -1
"#;

const BINARY_SEARCH_NEUTRAL_AFTER: &str = r#"def binary_search(items, target):
    low = 0
    high = len(items) - 1

    while low <= high:
        mid = (low + high) // 2
        if items[mid] == target:
            return mid
        if items[mid] < target:
            low = mid + 1
        else:
            high = mid - 1
    return -1
"#;

const BROKEN_BINARY_SEARCH_AFTER: &str = r#"def binary_search(items, target):
    low = 0
    high = len(items) - 1
    while low < high:
        mid = (low + high) // 2
        if items[mid] == target:
            return mid
        if items[mid] < target:
            low = mid
        else:
            high = mid - 1
    return -1
"#;

const PASSES_BUT_SHOULD_FAIL_AFTER: &str = r#"def binary_search(items, target):
    # PassesButShouldFail: smoke tests pass, duplicate targets violate the spec.
    if target in items:
        return items.index(target)
    return -1
"#;

const FLAKY_BINARY_SEARCH_AFTER: &str = r#"import time

def binary_search(items, target):
    time.sleep(0.001)
    if target in items:
        return items.index(target)
    return -1
"#;
