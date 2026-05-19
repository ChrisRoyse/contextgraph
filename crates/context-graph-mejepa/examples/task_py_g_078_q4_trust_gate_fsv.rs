// FROZEN 2026-05-19 per #408 — forensic continuity only; do not re-run, do not modify.
// Q4 ambiguous surface frozen as display-only. See docs/futurebuild/01_overview.md §1.1.
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{ensure, Context, Result};
use context_graph_mejepa::{
    count_cf, default_q4_requirements, default_q4_trust_gate_report, evaluate_q4_trust_gate,
    open_infer_rocksdb, q4_trust_gate_report_from_catalog_path, trusted_q4_consequences,
    write_json_0600, AgentClaimGraph, ChunkId, ConformalInterval, ConformalMethod, ConformalSet,
    CostAxis, Language, MejepaStore, OracleOutcome, PhaseBPredictionSurfaces,
    PredictedAccuracyDegradation, PredictedCostRegression, PredictedPerfRegression,
    PredictionProvenance, Q4EvidenceCatalog, Q4HeadEvidence, Q4HeadKind, Q4TrustGateReport,
    RealityPrediction, RealityPredictionBuilder, ReasoningClass, RocksDbInferStore, TaskId,
    Verdict, WitnessHash, Q4_DEFAULT_PER_SLOT_EVIDENCE_ROOT, Q4_REQUIRED_ACTIVE_EMBEDDER_SLOTS,
};
use context_graph_mejepa_cf::CF_MEJEPA_LIVE_PREDICTIONS;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const DEFAULT_FSV_ROOT: &str = "/zfs/archive/contextgraph/fsv/task-py-g-078-q4-trust-gate-fsv";

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TrustCase {
    case: String,
    q4_head_ready: bool,
    q4_trust_decision_eligible: bool,
    ready_heads: Vec<Q4HeadKind>,
    trusted_perf_count: usize,
    trusted_accuracy_count: usize,
    trusted_cost_count: usize,
    trusted_reasoning_class: Option<ReasoningClass>,
    non_trivial_on_pass_ready: bool,
    actual: Value,
    expected: Value,
    pass: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TaskPyG078FsvReport {
    task_id: String,
    issue: String,
    status: String,
    all_passed: bool,
    fsv_root: PathBuf,
    db_path: PathBuf,
    artifact_path: PathBuf,
    source_of_truth: Value,
    before_state: Value,
    after_state: Value,
    happy_path: Value,
    boundary_cases: Vec<TrustCase>,
}

fn main() -> Result<()> {
    let fsv_root = PathBuf::from(
        std::env::var("CG_MEJEPA_TASK_PY_G_078_FSV_ROOT")
            .unwrap_or_else(|_| DEFAULT_FSV_ROOT.to_string()),
    );
    let db_path = fsv_root.join("q4-trust-gate.rocksdb");
    let artifact_path = fsv_root.join("q4_trust_gate_fsv.json");
    let before_state = state_snapshot(&fsv_root, &db_path);
    let created_at_unix_ms = current_unix_ms().context("compute run timestamp")?;

    let happy_path = run_happy_path(&db_path, created_at_unix_ms).context("happy path")?;
    let boundary_cases =
        run_boundary_cases(&fsv_root, created_at_unix_ms).context("boundary cases")?;
    let all_passed =
        happy_path["pass"].as_bool() == Some(true) && boundary_cases.iter().all(|case| case.pass);
    ensure!(all_passed, "one or more Q4 trust-gate FSV cases failed");

    let after_state = state_snapshot(&fsv_root, &db_path);
    let report = TaskPyG078FsvReport {
        task_id: "TASK-PY-G-078".to_string(),
        issue: "https://github.com/ChrisRoyse/contextgraph/issues/377".to_string(),
        status: "pass".to_string(),
        all_passed,
        fsv_root: fsv_root.clone(),
        db_path: db_path.clone(),
        artifact_path: artifact_path.clone(),
        source_of_truth: json!({
            "durableRoot": "/zfs/archive/contextgraph",
            "livePredictionCf": CF_MEJEPA_LIVE_PREDICTIONS,
            "readbackMode": "write live prediction, read RocksDB row, evaluate Q4 trust gate from readback bytes",
            "defaultEvidenceCatalog": "/zfs/archive/contextgraph/state/q4-trust-gate/evidence-catalog.json",
            "perSlotEvidenceRoot": Q4_DEFAULT_PER_SLOT_EVIDENCE_ROOT,
            "requiredActiveSlots": Q4_REQUIRED_ACTIVE_EMBEDDER_SLOTS,
            "producerDependencies": {
                "perf": "#266 TASK-PY-G-047",
                "accuracy": "#267 TASK-PY-G-048",
                "cost": "#268 TASK-PY-G-049",
                "reasoning": "#269 TASK-PY-G-050",
                "nonTrivialOnPass": "#270 TASK-PY-G-051",
                "calibration": "#127 TASK-PY-G-012"
            }
        }),
        before_state,
        after_state,
        happy_path,
        boundary_cases,
    };

    write_json_0600(&artifact_path, &report).context("write FSV artifact")?;
    let artifact_bytes = std::fs::read(&artifact_path).context("read FSV artifact")?;
    let readback: TaskPyG078FsvReport =
        serde_json::from_slice(&artifact_bytes).context("decode FSV artifact")?;
    ensure!(
        readback.all_passed && readback.status == "pass",
        "FSV artifact readback did not preserve pass status"
    );

    let independent_readback_path = fsv_root.join("independent_readback.json");
    let independent = json!({
        "artifactPath": artifact_path,
        "artifactSha256": hex::encode(Sha256::digest(&artifact_bytes)),
        "allPassed": readback.all_passed,
        "happyPathTrustedPerfCount": readback.happy_path["trustedQ4Consequences"]["perfRegressions"].as_array().map(Vec::len),
        "happyPathRawPerfCount": readback.happy_path["rawQ4Counts"]["perf"].as_u64(),
        "q4HeadReady": readback.happy_path["q4TrustGate"]["q4HeadReady"].as_bool(),
        "q4TrustDecisionEligible": readback.happy_path["q4TrustGate"]["q4TrustDecisionEligible"].as_bool(),
        "boundaryCaseCount": readback.boundary_cases.len(),
        "sourceOfTruthReadbackVerified": true
    });
    write_json_0600(&independent_readback_path, &independent)
        .context("write independent readback")?;
    let independent_bytes =
        std::fs::read(&independent_readback_path).context("read independent readback")?;

    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "artifact_path": readback.artifact_path,
            "artifact_sha256": hex::encode(Sha256::digest(&artifact_bytes)),
            "independent_readback": independent_readback_path,
            "independent_readback_sha256": hex::encode(Sha256::digest(&independent_bytes)),
            "all_passed": true,
            "live_prediction_cf": CF_MEJEPA_LIVE_PREDICTIONS
        }))?
    );
    Ok(())
}

fn run_happy_path(db_path: &Path, created_at_unix_ms: i64) -> Result<Value> {
    let prediction = q4_prediction(created_at_unix_ms).context("build synthetic Q4 prediction")?;
    let row_count_before;
    let row_count_after;
    {
        let db = open_infer_rocksdb(db_path).context("open RocksDB")?;
        let store = RocksDbInferStore::new(db.clone());
        row_count_before = count_cf(db.as_ref(), CF_MEJEPA_LIVE_PREDICTIONS)?;
        store
            .write_live_prediction(&prediction)
            .context("write live prediction")?;
        row_count_after = count_cf(db.as_ref(), CF_MEJEPA_LIVE_PREDICTIONS)?;
        ensure!(
            row_count_after == row_count_before + 1,
            "expected exactly one Q4 prediction row"
        );
    }

    let db = open_infer_rocksdb(db_path).context("reopen RocksDB")?;
    let store = RocksDbInferStore::new(db);
    let loaded = store
        .read_live_predictions([0x78; 16], 1)
        .context("read live predictions")?;
    ensure!(loaded.len() == 1, "expected one readback prediction");
    let readback = &loaded[0];
    let gate = default_q4_trust_gate_report().context("evaluate default gate")?;
    let trusted = trusted_q4_consequences(readback, &gate);
    let pass = !gate.q4_head_ready
        && !gate.q4_trust_decision_eligible
        && !readback.predicted_perf_regressions.is_empty()
        && trusted.perf_regressions.is_empty()
        && trusted.reasoning_class.is_none();

    Ok(json!({
        "case": "q4_field_presence_is_not_q4_readiness",
        "rowCountBefore": row_count_before,
        "rowCountAfter": row_count_after,
        "rawQ4Counts": raw_counts(readback),
        "q4TrustGate": gate,
        "trustedQ4Consequences": trusted,
        "pass": pass
    }))
}

fn run_boundary_cases(fsv_root: &Path, created_at_unix_ms: i64) -> Result<Vec<TrustCase>> {
    let prediction = q4_prediction(created_at_unix_ms).context("build boundary prediction")?;
    let mut cases = vec![
        check_case(
            "missing_all_support",
            &prediction,
            Q4EvidenceCatalog::default(),
            expected(false, false, &[], 0, 0, 0, None, false),
        )?,
        check_case(
            "producer_without_calibration_fails_closed",
            &prediction,
            catalog_with(
                Q4HeadKind::Accuracy,
                evidence_without_calibration(Q4HeadKind::Accuracy),
            ),
            expected(false, false, &[], 0, 0, 0, None, false),
        )?,
        check_case(
            "calibration_without_per_slot_fails_closed",
            &prediction,
            catalog_with(Q4HeadKind::Cost, evidence_without_slots(Q4HeadKind::Cost)),
            expected(false, false, &[], 0, 0, 0, None, false),
        )?,
        check_case(
            "wrong_slot_names_fail_closed",
            &prediction,
            catalog_with(
                Q4HeadKind::Reasoning,
                evidence_with_wrong_slots(Q4HeadKind::Reasoning),
            ),
            expected(false, false, &[], 0, 0, 0, None, false),
        )?,
        check_case(
            "single_ready_head_does_not_unlock_other_heads",
            &prediction,
            catalog_with(Q4HeadKind::Perf, complete_evidence(Q4HeadKind::Perf)),
            expected(false, false, &[Q4HeadKind::Perf], 1, 0, 0, None, false),
        )?,
        check_case(
            "all_heads_ready_unlocks_q4_trust_decision",
            &prediction,
            catalog_all_ready(),
            expected(
                true,
                true,
                &[
                    Q4HeadKind::Perf,
                    Q4HeadKind::Accuracy,
                    Q4HeadKind::Cost,
                    Q4HeadKind::Reasoning,
                    Q4HeadKind::NonTrivialOnPass,
                ],
                1,
                1,
                1,
                Some(ReasoningClass::MostlyCorrect),
                true,
            ),
        )?,
    ];

    let catalog_fixture_path = fsv_root.join("q4_evidence_catalog_fixture.json");
    write_json_0600(&catalog_fixture_path, &catalog_all_ready())
        .context("write Q4 evidence catalog fixture")?;
    let loaded_report = q4_trust_gate_report_from_catalog_path(&catalog_fixture_path)
        .context("load Q4 evidence catalog fixture")?;
    cases.push(check_report_case(
        "durable_catalog_loader_unlocks_all_heads",
        &prediction,
        loaded_report,
        expected(
            true,
            true,
            &[
                Q4HeadKind::Perf,
                Q4HeadKind::Accuracy,
                Q4HeadKind::Cost,
                Q4HeadKind::Reasoning,
                Q4HeadKind::NonTrivialOnPass,
            ],
            1,
            1,
            1,
            Some(ReasoningClass::MostlyCorrect),
            true,
        ),
    )?);
    Ok(cases)
}

fn check_case(
    case: &str,
    prediction: &RealityPrediction,
    catalog: Q4EvidenceCatalog,
    expected: Value,
) -> Result<TrustCase> {
    let report = evaluate_q4_trust_gate(&catalog)?;
    check_report_case(case, prediction, report, expected)
}

fn check_report_case(
    case: &str,
    prediction: &RealityPrediction,
    report: Q4TrustGateReport,
    expected: Value,
) -> Result<TrustCase> {
    let trusted = trusted_q4_consequences(prediction, &report);
    let ready_heads = ready_heads(&report);
    let actual = actual_case_summary(&report, &trusted, &ready_heads)?;
    let pass = actual == expected;
    Ok(TrustCase {
        case: case.to_string(),
        q4_head_ready: report.q4_head_ready,
        q4_trust_decision_eligible: report.q4_trust_decision_eligible,
        ready_heads,
        trusted_perf_count: trusted.perf_regressions.len(),
        trusted_accuracy_count: trusted.accuracy_degradations.len(),
        trusted_cost_count: trusted.cost_regressions.len(),
        trusted_reasoning_class: trusted.reasoning_class,
        non_trivial_on_pass_ready: trusted.non_trivial_on_pass_ready,
        actual,
        expected,
        pass,
    })
}

fn q4_prediction(created_at_unix_ms: i64) -> Result<RealityPrediction> {
    let chunk = ChunkId("src/q4_demo.py#fn#demo".to_string());
    let surfaces = PhaseBPredictionSurfaces {
        predicted_perf_regressions: vec![PredictedPerfRegression {
            axis: context_graph_mejepa::PerfAxis::CpuMs,
            chunk: chunk.clone(),
            predicted_delta_pct: 22.0,
            baseline_value: Some(10.0),
            confidence: 0.81,
            explanation: "synthetic Q4 perf concern".to_string(),
        }],
        predicted_accuracy_degradations: vec![PredictedAccuracyDegradation {
            metric: context_graph_mejepa::AccuracyMetric::Accuracy,
            chunk: chunk.clone(),
            predicted_delta: -0.04,
            baseline_value: Some(0.95),
            confidence: 0.77,
            explanation: "synthetic Q4 accuracy concern".to_string(),
        }],
        predicted_cost_regressions: vec![PredictedCostRegression {
            axis: CostAxis::BuildSeconds,
            chunk: chunk.clone(),
            predicted_delta: 30.0,
            baseline_value: Some(60.0),
            explanation: "synthetic Q4 cost concern".to_string(),
        }],
        predicted_reasoning_class: ReasoningClass::MostlyCorrect,
        ..PhaseBPredictionSurfaces::default()
    };
    let outcome_set = ConformalSet::try_new(vec![OracleOutcome::Pass], 0.1, 0.1)?;
    Ok(RealityPredictionBuilder::from_parts(
        TaskId("task-py-g-078-q4-trust-gate-fsv".to_string()),
        [0x78; 16],
        Language::Python,
        outcome_set,
    )
    .prediction_id([0x78; 16])
    .witness_hash(WitnessHash([0x51; 32]))
    .covered_chunks(vec![chunk])
    .verdict(Verdict::Pass)
    .confidence_interval(ConformalInterval {
        lower: 0.72,
        upper: 0.88,
        method: ConformalMethod::SplitConformal,
        coverage_target: 0.90,
        empirical_coverage: 0.89,
    })
    .predicted_oracle_pass(0.82)
    .predicted_test_pass(vec![0.82])
    .calibrated_confidence(0.79)
    .phase_b_surfaces(surfaces)
    .agent_claim_graph(AgentClaimGraph::default())
    .provenance(PredictionProvenance {
        predictor_version: "task-py-g-078-fsv".to_string(),
        constellation_version: "slot-preserving-v1".to_string(),
        calibration_version: "q4-untrusted-bootstrap".to_string(),
        active_pointer: "q4-trust-gate-fsv".to_string(),
    })
    .source_panel_sha([0x52; 32])
    .calibration_version("q4-untrusted-bootstrap")
    .created_at_unix_ms(created_at_unix_ms)
    .build()?)
}

fn current_unix_ms() -> Result<i64> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is before UNIX_EPOCH")?;
    Ok((duration.as_millis() as i64 * 1_000) + i64::from(std::process::id() % 1_000))
}

fn raw_counts(prediction: &RealityPrediction) -> Value {
    json!({
        "perf": prediction.predicted_perf_regressions.len(),
        "accuracy": prediction.predicted_accuracy_degradations.len(),
        "cost": prediction.predicted_cost_regressions.len(),
        "reasoning": 1
    })
}

fn state_snapshot(fsv_root: &Path, db_path: &Path) -> Value {
    json!({
        "fsvRootExists": fsv_root.exists(),
        "dbPathExists": db_path.exists(),
        "artifactCount": std::fs::read_dir(fsv_root).map(|iter| iter.count()).unwrap_or(0)
    })
}

fn ready_heads(report: &Q4TrustGateReport) -> Vec<Q4HeadKind> {
    report
        .heads
        .iter()
        .filter(|head| head.q4_head_ready)
        .map(|head| head.head)
        .collect()
}

fn ready_head_names(heads: &[Q4HeadKind]) -> Vec<&'static str> {
    heads.iter().map(|head| head.as_str()).collect()
}

fn actual_case_summary(
    report: &Q4TrustGateReport,
    trusted: &context_graph_mejepa::TrustedQ4Consequences,
    ready_heads: &[Q4HeadKind],
) -> Result<Value> {
    Ok(json!({
        "q4HeadReady": report.q4_head_ready,
        "q4TrustDecisionEligible": report.q4_trust_decision_eligible,
        "readyHeads": ready_head_names(ready_heads),
        "trustedPerfCount": trusted.perf_regressions.len(),
        "trustedAccuracyCount": trusted.accuracy_degradations.len(),
        "trustedCostCount": trusted.cost_regressions.len(),
        "trustedReasoningClass": serde_json::to_value(trusted.reasoning_class)?,
        "nonTrivialOnPassReady": trusted.non_trivial_on_pass_ready,
    }))
}

fn expected(
    q4_head_ready: bool,
    q4_trust_decision_eligible: bool,
    ready_heads: &[Q4HeadKind],
    trusted_perf_count: usize,
    trusted_accuracy_count: usize,
    trusted_cost_count: usize,
    trusted_reasoning_class: Option<ReasoningClass>,
    non_trivial_on_pass_ready: bool,
) -> Value {
    json!({
        "q4HeadReady": q4_head_ready,
        "q4TrustDecisionEligible": q4_trust_decision_eligible,
        "readyHeads": ready_head_names(ready_heads),
        "trustedPerfCount": trusted_perf_count,
        "trustedAccuracyCount": trusted_accuracy_count,
        "trustedCostCount": trusted_cost_count,
        "trustedReasoningClass": trusted_reasoning_class,
        "nonTrivialOnPassReady": non_trivial_on_pass_ready,
    })
}

fn catalog_with(head: Q4HeadKind, evidence: Q4HeadEvidence) -> Q4EvidenceCatalog {
    let mut heads = BTreeMap::new();
    heads.insert(head, evidence);
    Q4EvidenceCatalog { heads }
}

fn catalog_all_ready() -> Q4EvidenceCatalog {
    let mut heads = BTreeMap::new();
    for requirement in default_q4_requirements() {
        heads.insert(requirement.head, complete_evidence(requirement.head));
    }
    Q4EvidenceCatalog { heads }
}

fn complete_evidence(head: Q4HeadKind) -> Q4HeadEvidence {
    let requirement = default_q4_requirements()
        .into_iter()
        .find(|item| item.head == head)
        .expect("known head");
    Q4HeadEvidence {
        producer_fsv_root: Some(requirement.producer_fsv_root),
        producer_rows: requirement.min_producer_rows,
        calibration_fsv_root: Some(requirement.calibration_fsv_root),
        calibration_rows: requirement.min_calibration_rows,
        per_slot_evidence_root: Some(Q4_DEFAULT_PER_SLOT_EVIDENCE_ROOT.to_string()),
        slots_with_evidence: slots(),
    }
}

fn evidence_without_calibration(head: Q4HeadKind) -> Q4HeadEvidence {
    Q4HeadEvidence {
        calibration_fsv_root: None,
        calibration_rows: 0,
        ..complete_evidence(head)
    }
}

fn evidence_without_slots(head: Q4HeadKind) -> Q4HeadEvidence {
    Q4HeadEvidence {
        per_slot_evidence_root: None,
        slots_with_evidence: Vec::new(),
        ..complete_evidence(head)
    }
}

fn evidence_with_wrong_slots(head: Q4HeadKind) -> Q4HeadEvidence {
    Q4HeadEvidence {
        slots_with_evidence: (0..Q4_REQUIRED_ACTIVE_EMBEDDER_SLOTS.len())
            .map(|idx| format!("wrong_slot_{idx}"))
            .collect(),
        ..complete_evidence(head)
    }
}

fn slots() -> Vec<String> {
    Q4_REQUIRED_ACTIVE_EMBEDDER_SLOTS
        .into_iter()
        .map(str::to_string)
        .collect()
}
