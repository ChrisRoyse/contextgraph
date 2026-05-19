use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{ensure, Context, Result};
use context_graph_mejepa::cli::write_json_0600;
use context_graph_mejepa::{
    build_hierarchical_prediction, count_cf, open_infer_rocksdb, sha256_bytes,
    valid_witness_segment, AccuracyMetric, AstDiff, ChunkId, ConformalInterval, ConformalSet,
    CostAxis, DiffHunk, EmbedderId, FailureModeClass, GuardViolation, Language,
    MatchedFingerprintEvidence, MejepaStore, OracleOutcome, PatchBundle, PerSlotOodReason,
    PerSlotOodReasonKind, PerfAxis, PhaseBPredictionSurfaces, PredictedAccuracyDegradation,
    PredictedCostRegression, PredictedFailureMode, PredictedPerfRegression,
    PredictedSecurityConcern, PredictedWorks, PredictionCorrectness, PredictionProvenance,
    RealityImpact, RealityPrediction, RealityPredictionBuilder, ReasoningClass, RocksDbInferStore,
    RootCauseClass, SecurityConcernClass, Severity, ShiftEntry, SlotAttributionEvidence,
    SlotAttributionPolarity, SlotAttributionSource, TaskId, Verdict, WitnessHash,
    SLOT_ATTRIBUTION_SCHEMA_VERSION,
};
use context_graph_mejepa_cf::{CF_MEJEPA_HIERARCHICAL_PREDICTIONS, CF_MEJEPA_LIVE_PREDICTIONS};
use context_graph_mejepa_instruments::{InstrumentSlot, Panel, PANEL_DIM};
use rocksdb::{IteratorMode, DB};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const DEFAULT_FSV_ROOT: &str =
    "/zfs/archive/contextgraph/fsv/task-py-g-091-slot-attribution-explanation-fsv";
const SESSION_ID: [u8; 16] = [0x91; 16];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct EdgeCase {
    name: String,
    pass: bool,
    evidence: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TaskPyG091FsvReport {
    task_id: String,
    issue: u32,
    status: String,
    all_passed: bool,
    source_of_truth: Value,
    prediction_summaries: Value,
    persistence_readback: Value,
    hierarchy_readback: Value,
    mcp_surface_projection: Value,
    edge_cases: Vec<EdgeCase>,
    independent_readback: Value,
}

fn main() -> Result<()> {
    let run_id = format!("run-{}", chrono::Utc::now().format("%Y%m%dT%H%M%SZ"));
    let fsv_root = PathBuf::from(
        std::env::var("CG_TASK_PY_G_091_FSV_ROOT").unwrap_or_else(|_| DEFAULT_FSV_ROOT.to_string()),
    );
    let run_root = fsv_root.join(run_id);
    fs::create_dir_all(&run_root)?;
    let db_path = run_root.join("slot-attribution.rocksdb");

    let patch = one_hunk_patch()?;
    let panel = deterministic_panel()?;
    let pass_prediction = pass_prediction()?;
    let rejected_prediction = rejected_prediction()?;
    let unknown_prediction = unknown_prediction()?;
    let pass_hierarchy = build_hierarchical_prediction(&pass_prediction, &patch, &panel)?;
    let rejected_hierarchy = build_hierarchical_prediction(&rejected_prediction, &patch, &panel)?;

    let db = open_infer_rocksdb(&db_path)?;
    let store = RocksDbInferStore::new(db.clone());
    store.write_live_prediction(&pass_prediction)?;
    store.write_live_prediction(&rejected_prediction)?;
    store.write_live_prediction(&unknown_prediction)?;
    store.write_hierarchical_prediction(&pass_hierarchy)?;
    store.write_hierarchical_prediction(&rejected_hierarchy)?;

    let readback_predictions = store.read_live_predictions(SESSION_ID, 8)?;
    let readback_hierarchies = store.read_hierarchical_predictions(SESSION_ID, 8)?;
    let prediction_summaries = json!({
        "pass": attribution_summary(&pass_prediction),
        "rejected": attribution_summary(&rejected_prediction),
        "unknown": attribution_summary(&unknown_prediction),
    });
    let persistence_readback = json!({
        "pass": readback_predictions.len() == 3
            && readback_predictions.iter().any(|row| row == &pass_prediction)
            && readback_predictions.iter().any(|row| row == &rejected_prediction)
            && readback_predictions.iter().any(|row| row == &unknown_prediction)
            && readback_predictions.iter().all(|row| !row.slot_attributions.is_empty()),
        "predictionCount": readback_predictions.len(),
        "slotAttributionCounts": readback_predictions
            .iter()
            .map(|row| (hex::encode(row.prediction_id), row.slot_attributions.len()))
            .collect::<BTreeMap<_, _>>(),
    });
    let hierarchy_readback = json!({
        "pass": readback_hierarchies.len() == 2
            && readback_hierarchies.iter().all(|row| !row.slot_attributions.is_empty())
            && readback_hierarchies.iter().any(|row| row.slot_attributions == pass_prediction.slot_attributions)
            && readback_hierarchies.iter().any(|row| row.slot_attributions == rejected_prediction.slot_attributions),
        "hierarchyCount": readback_hierarchies.len(),
        "hierarchySlotAttributionCounts": readback_hierarchies
            .iter()
            .map(|row| (hex::encode(row.prediction_id), row.slot_attributions.len()))
            .collect::<BTreeMap<_, _>>(),
    });
    let mcp_surface_projection = mcp_surface_projection(&rejected_prediction)?;
    let edge_cases = edge_cases()?;
    ensure!(edge_cases.iter().all(|case| case.pass), "edge case failed");

    drop(store);
    db.cancel_all_background_work(true);
    drop(db);

    let reopened = open_infer_rocksdb(&db_path)?;
    let reopened_store = RocksDbInferStore::new(reopened.clone());
    let reopened_predictions = reopened_store.read_live_predictions(SESSION_ID, 8)?;
    let reopened_hierarchies = reopened_store.read_hierarchical_predictions(SESSION_ID, 8)?;
    let independent_readback = json!({
        "allPassed": true,
        "dbPath": db_path,
        "livePredictionRows": count_cf(reopened.as_ref(), CF_MEJEPA_LIVE_PREDICTIONS)?,
        "hierarchicalPredictionRows": count_cf(reopened.as_ref(), CF_MEJEPA_HIERARCHICAL_PREDICTIONS)?,
        "livePredictionRowsViaStore": reopened_predictions.len(),
        "hierarchicalPredictionRowsViaStore": reopened_hierarchies.len(),
        "livePredictionSlotAttributionsPersisted": reopened_predictions.iter().all(|row| !row.slot_attributions.is_empty()),
        "hierarchicalSlotAttributionsPersisted": reopened_hierarchies.iter().all(|row| !row.slot_attributions.is_empty()),
        "livePredictionCfPhysicalRows": count_physical_cf(reopened.as_ref(), CF_MEJEPA_LIVE_PREDICTIONS)?,
        "hierarchicalPredictionCfPhysicalRows": count_physical_cf(reopened.as_ref(), CF_MEJEPA_HIERARCHICAL_PREDICTIONS)?,
    });
    ensure!(
        independent_readback["livePredictionRows"].as_u64() == Some(3)
            && independent_readback["hierarchicalPredictionRows"].as_u64() == Some(2)
            && independent_readback["livePredictionSlotAttributionsPersisted"] == true
            && independent_readback["hierarchicalSlotAttributionsPersisted"] == true,
        "independent RocksDB readback counts mismatch"
    );

    let all_passed = prediction_summaries["pass"]["hasSupportingSlot"] == true
        && prediction_summaries["pass"]["hasFailureFingerprint"] == true
        && prediction_summaries["rejected"]["hasRejectionEvidence"] == true
        && prediction_summaries["rejected"]["hasQ4Concern"] == true
        && prediction_summaries["rejected"]["hasQ5Impact"] == true
        && prediction_summaries["unknown"]["hasActiveLearningCandidate"] == true
        && persistence_readback["pass"] == true
        && hierarchy_readback["pass"] == true
        && mcp_surface_projection["pass"] == true
        && edge_cases.iter().all(|case| case.pass)
        && independent_readback["allPassed"] == true;

    let report = TaskPyG091FsvReport {
        task_id: "TASK-PY-G-091".to_string(),
        issue: 391,
        status: if all_passed { "pass" } else { "fail" }.to_string(),
        all_passed,
        source_of_truth: json!({
            "durableRoot": "/zfs/archive/contextgraph",
            "fsvRoot": run_root,
            "livePredictionCf": CF_MEJEPA_LIVE_PREDICTIONS,
            "hierarchicalPredictionCf": CF_MEJEPA_HIERARCHICAL_PREDICTIONS,
            "predictionField": "slot_attributions",
            "policy": "agent-facing explanations are compact views over full persisted per-slot attribution records",
        }),
        prediction_summaries,
        persistence_readback,
        hierarchy_readback,
        mcp_surface_projection,
        edge_cases,
        independent_readback,
    };
    let report_path = run_root.join("slot_attribution_explanation_fsv.json");
    write_json_0600(&report_path, &report)?;
    let readback: TaskPyG091FsvReport = serde_json::from_slice(&fs::read(&report_path)?)?;
    ensure!(
        readback.all_passed == report.all_passed
            && readback.prediction_summaries == report.prediction_summaries
            && readback.independent_readback == report.independent_readback,
        "FSV report durable readback mismatch"
    );
    ensure!(report.all_passed, "TASK-PY-G-091 FSV did not pass");
    println!(
        "task-py-g-091 slot attribution explanation FSV allPassed=true report={} sha256={}",
        report_path.display(),
        sha256_file(&report_path)?
    );
    Ok(())
}

fn one_hunk_patch() -> Result<PatchBundle> {
    let after = "def normalize_user(value):\n    if value is None:\n        return ''\n    return str(value).strip().lower()\n";
    Ok(PatchBundle::try_new(
        AstDiff {
            hunks: vec![DiffHunk {
                path: "src/user.py".into(),
                pre_sha: sha256_bytes(b""),
                post_sha: sha256_bytes(after.as_bytes()),
                before: String::new(),
                after: after.to_string(),
            }],
        },
        valid_witness_segment(),
        "TASK-PY-G-091 slot attribution explanation FSV".to_string(),
        sha256_bytes(b"task-py-g-091-slot-attribution-source-panel"),
    )?)
}

fn pass_prediction() -> Result<RealityPrediction> {
    let chunk = chunk();
    Ok(base_builder(
        [0x10; 16],
        Verdict::Pass,
        vec![OracleOutcome::Pass],
        0.91,
        0.87,
        0.08,
    )?
    .predicted_works(vec![PredictedWorks {
        chunk: chunk.clone(),
        line_range: (1, 4),
        claim: "normalization branch exists on disk and handles None".to_string(),
        confidence: 0.92,
        supporting_embedders: vec![
            EmbedderId("E_AST".to_string()),
            EmbedderId("E_SPARSE".to_string()),
        ],
        similar_known_good_exemplars: Vec::new(),
        evidence_strength: 0.90,
    }])
    .matched_fingerprint(Some(MatchedFingerprintEvidence {
        fingerprint_id: sha256_bytes(b"task-py-g-091-known-good-fingerprint"),
        name: "known_good_python_normalization_guard".to_string(),
        kind_slug: "known_good".to_string(),
        oracle_outcome: Some(OracleOutcome::Pass),
        mean_cosine: 0.94,
        min_margin: 0.08,
        n_references: 7,
        calibration_state: "calibrated".to_string(),
    }))
    .build()?)
}

fn rejected_prediction() -> Result<RealityPrediction> {
    let chunk = chunk();
    let mut surfaces = PhaseBPredictionSurfaces::default();
    surfaces.predicted_failure_modes = vec![PredictedFailureMode {
        failure_class: FailureModeClass::ApiMisuse,
        chunk: chunk.clone(),
        line_range: (2, 3),
        confidence: 0.86,
        severity: Severity::High,
        explanation: "slot E_GRAPH shows this branch bypasses the repository auth boundary"
            .to_string(),
        contributing_embedders: vec![EmbedderId("E_GRAPH".to_string())],
        root_cause_class: RootCauseClass::InterfaceError,
    }];
    surfaces.predicted_perf_regressions = vec![PredictedPerfRegression {
        axis: PerfAxis::P99LatencyMs,
        chunk: chunk.clone(),
        predicted_delta_pct: 14.0,
        baseline_value: Some(21.0),
        confidence: 0.74,
        explanation: "normalization now runs on the hot request path".to_string(),
    }];
    surfaces.predicted_security_concerns = vec![PredictedSecurityConcern {
        class: SecurityConcernClass::BrokenAccessControl,
        chunk: chunk.clone(),
        line_range: (2, 4),
        cvss_estimate: Some(7.2),
        explanation: "changed branch can transform auth-sensitive identifiers before policy lookup"
            .to_string(),
    }];
    surfaces.predicted_accuracy_degradations = vec![PredictedAccuracyDegradation {
        metric: AccuracyMetric::Accuracy,
        chunk: chunk.clone(),
        predicted_delta: -0.06,
        baseline_value: Some(0.97),
        confidence: 0.71,
        explanation: "case-folding loses signal for case-sensitive identifiers".to_string(),
    }];
    surfaces.predicted_cost_regressions = vec![PredictedCostRegression {
        axis: CostAxis::CiWallSeconds,
        chunk: chunk.clone(),
        predicted_delta: 9.0,
        baseline_value: Some(42.0),
        explanation: "larger blast radius requires additional tests in CI".to_string(),
    }];
    surfaces.predicted_reasoning_class = ReasoningClass::Overclaiming;

    Ok(base_builder(
        [0x20; 16],
        Verdict::GuardRejected,
        vec![OracleOutcome::Fail],
        0.18,
        0.79,
        0.88,
    )?
    .phase_b_surfaces(surfaces)
    .guard_violations(vec![GuardViolation {
        embedder: EmbedderId("E_GRAPH".to_string()),
        chunk: chunk.clone(),
        centroid_id: "known_good:python:function:e_graph".to_string(),
        cosine: 0.61,
        threshold_tau_m: 0.84,
        deficit: 0.23,
    }])
    .per_slot_ood_reasons(vec![PerSlotOodReason {
        embedder: EmbedderId("E_GRAPH".to_string()),
        chunk: Some(chunk.clone()),
        reason: PerSlotOodReasonKind::GtauViolation,
        observed_score: 0.61,
        threshold: 0.84,
        margin: 0.23,
        calibration_version: "task-py-g-091-calibration-v1".to_string(),
        evidence: "E_GRAPH moved outside its per-slot calibrated guard band".to_string(),
    }])
    .reality_impact(Some(RealityImpact {
        observed_shifts: vec![ShiftEntry {
            shift_id: "task-py-g-091-shift-1".to_string(),
            file: "src/user.py".into(),
            before_sha: sha256_bytes(b""),
            after_sha: sha256_bytes(b"normalize_user"),
        }],
        predicted_files_changed: vec!["src/user.py".into()],
        observed_files_changed: vec!["src/user.py".into()],
        unexpected_files_changed: Vec::new(),
        predicted_test_outcomes: Vec::new(),
        observed_test_outcomes: Vec::new(),
        prediction_correctness: PredictionCorrectness::Aligned,
    }))
    .build()?)
}

fn unknown_prediction() -> Result<RealityPrediction> {
    Ok(base_builder(
        [0x30; 16],
        Verdict::OutOfDistribution,
        vec![OracleOutcome::OutOfDistribution],
        0.44,
        0.51,
        0.93,
    )?
    .unknown_candidate_id(Some([0x39; 16]))
    .per_slot_ood_reasons(vec![PerSlotOodReason {
        embedder: EmbedderId("E_TEMPORAL".to_string()),
        chunk: Some(chunk()),
        reason: PerSlotOodReasonKind::MissingSlotObservation,
        observed_score: 0.0,
        threshold: 0.70,
        margin: 0.70,
        calibration_version: "task-py-g-091-calibration-v1".to_string(),
        evidence: "temporal slot is missing for this code state".to_string(),
    }])
    .build()?)
}

fn base_builder(
    prediction_id: [u8; 16],
    verdict: Verdict,
    outcomes: Vec<OracleOutcome>,
    predicted_oracle_pass: f32,
    calibrated_confidence: f32,
    ood_score: f32,
) -> Result<RealityPredictionBuilder> {
    Ok(RealityPredictionBuilder::from_parts(
        TaskId("task-py-g-091-slot-attribution-explanation-fsv".to_string()),
        SESSION_ID,
        Language::Python,
        ConformalSet::try_new(outcomes, 0.1, 0.2)?,
    )
    .prediction_id(prediction_id)
    .witness_hash(WitnessHash([0x91; 32]))
    .covered_chunks(vec![chunk()])
    .verdict(verdict)
    .confidence_interval(ConformalInterval {
        lower: (predicted_oracle_pass - 0.08).max(0.0),
        upper: (predicted_oracle_pass + 0.08).min(1.0),
        ..ConformalInterval::default()
    })
    .predicted_oracle_pass(predicted_oracle_pass)
    .predicted_test_pass(vec![predicted_oracle_pass])
    .ood_score(ood_score)
    .calibrated_confidence(calibrated_confidence)
    .granger_attestations(BTreeMap::from([(
        "dda:slot_panel_consistency".to_string(),
        0.82,
    )]))
    .provenance(PredictionProvenance {
        predictor_version: "task-py-g-091-fsv".to_string(),
        constellation_version: "task-py-g-091-fsv".to_string(),
        calibration_version: "task-py-g-091-calibration-v1".to_string(),
        active_pointer: hex::encode(prediction_id),
    })
    .source_panel_sha(sha256_bytes(b"task-py-g-091-source-panel"))
    .calibration_version("task-py-g-091-calibration-v1")
    .created_at_unix_ms(chrono::Utc::now().timestamp_millis()))
}

fn attribution_summary(prediction: &RealityPrediction) -> Value {
    let supporting = count_polarity(prediction, SlotAttributionPolarity::Supporting);
    let rejection = count_polarity(prediction, SlotAttributionPolarity::Violating)
        + count_polarity(prediction, SlotAttributionPolarity::Missing)
        + count_polarity(prediction, SlotAttributionPolarity::Stale);
    json!({
        "predictionId": hex::encode(prediction.prediction_id),
        "verdict": prediction.verdict,
        "count": prediction.slot_attributions.len(),
        "supportingCount": supporting,
        "rejectionEvidenceCount": rejection,
        "q4ConcernCount": count_source(prediction, SlotAttributionSource::Q4Head),
        "q5ImpactCount": count_source(prediction, SlotAttributionSource::Q5Replay),
        "failureFingerprintCount": count_source(prediction, SlotAttributionSource::FailureFingerprint),
        "activeLearningCount": count_source(prediction, SlotAttributionSource::ActiveLearningCandidate),
        "hasSupportingSlot": supporting > 0,
        "hasRejectionEvidence": rejection > 0,
        "hasQ4Concern": count_source(prediction, SlotAttributionSource::Q4Head) > 0,
        "hasQ5Impact": count_source(prediction, SlotAttributionSource::Q5Replay) > 0,
        "hasFailureFingerprint": count_source(prediction, SlotAttributionSource::FailureFingerprint) > 0,
        "hasActiveLearningCandidate": count_source(prediction, SlotAttributionSource::ActiveLearningCandidate) > 0,
        "topReasons": prediction.slot_attributions.iter().take(6).map(|row| row.reason.clone()).collect::<Vec<_>>(),
    })
}

fn mcp_surface_projection(prediction: &RealityPrediction) -> Result<Value> {
    let full_json = serde_json::to_value(prediction)?;
    let compact = prediction
        .slot_attributions
        .iter()
        .take(8)
        .map(|row| {
            json!({
                "slotId": row.slot_id,
                "polarity": row.polarity,
                "source": row.source,
                "score": row.score,
                "reason": row.reason,
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({
        "pass": full_json.get("slot_attributions").is_some()
            && !compact.is_empty()
            && compact.iter().all(|row| row.get("slotId").is_some() && row.get("reason").is_some()),
        "latestSurface": "slotAttributionSummaries",
        "explainSurface": "slotAttributionsCompact",
        "inspectSurface": "slotAttributions",
        "shipGateSurface": "slotAttributionSurface",
        "fullRecordHasSlotAttributions": full_json.get("slot_attributions").is_some(),
        "compact": compact,
    }))
}

fn edge_cases() -> Result<Vec<EdgeCase>> {
    let duplicate = {
        let attr = unit_attribution(SlotAttributionPolarity::Supporting);
        manual_prediction(
            [0xa1; 16],
            Verdict::Pass,
            vec![OracleOutcome::Pass],
            vec![attr.clone(), attr],
        )
        .is_err()
    };
    let rejection_without_rejection_evidence = manual_prediction(
        [0xa2; 16],
        Verdict::GuardRejected,
        vec![OracleOutcome::Fail],
        vec![unit_attribution(SlotAttributionPolarity::Supporting)],
    )
    .is_err();
    let zero_candidate_id = {
        let mut attr = unit_attribution(SlotAttributionPolarity::Missing);
        attr.source = SlotAttributionSource::ActiveLearningCandidate;
        attr.active_learning_candidate_id = Some([0; 16]);
        manual_prediction(
            [0xa3; 16],
            Verdict::OutOfDistribution,
            vec![OracleOutcome::OutOfDistribution],
            vec![attr],
        )
        .is_err()
    };
    Ok(vec![
        EdgeCase {
            name: "duplicate_slot_attribution_rejected".to_string(),
            pass: duplicate,
            evidence: json!({"expected": "MEJEPA_INFER_INVALID_INPUT"}),
        },
        EdgeCase {
            name: "rejection_verdict_requires_violating_missing_or_stale_slot".to_string(),
            pass: rejection_without_rejection_evidence,
            evidence: json!({"verdict": "guard_rejected"}),
        },
        EdgeCase {
            name: "active_learning_candidate_id_must_be_nonzero".to_string(),
            pass: zero_candidate_id,
            evidence: json!({"candidateId": hex::encode([0u8; 16])}),
        },
    ])
}

fn manual_prediction(
    prediction_id: [u8; 16],
    verdict: Verdict,
    outcomes: Vec<OracleOutcome>,
    slot_attributions: Vec<SlotAttributionEvidence>,
) -> Result<RealityPrediction, context_graph_mejepa::MejepaInferError> {
    RealityPredictionBuilder::from_parts(
        TaskId("task-py-g-091-edge-case".to_string()),
        [0xee; 16],
        Language::Python,
        ConformalSet::try_new(outcomes, 0.1, 0.2)?,
    )
    .prediction_id(prediction_id)
    .witness_hash(WitnessHash([0xee; 32]))
    .covered_chunks(vec![chunk()])
    .verdict(verdict)
    .confidence_interval(ConformalInterval {
        lower: 0.1,
        upper: 0.8,
        ..ConformalInterval::default()
    })
    .predicted_oracle_pass(0.5)
    .predicted_test_pass(vec![0.5])
    .ood_score(0.2)
    .calibrated_confidence(0.6)
    .provenance(PredictionProvenance {
        predictor_version: "task-py-g-091-edge".to_string(),
        constellation_version: "task-py-g-091-edge".to_string(),
        calibration_version: "task-py-g-091-edge".to_string(),
        active_pointer: hex::encode(prediction_id),
    })
    .source_panel_sha(sha256_bytes(b"task-py-g-091-edge-source-panel"))
    .calibration_version("task-py-g-091-edge")
    .slot_attributions(slot_attributions)
    .build()
}

fn unit_attribution(polarity: SlotAttributionPolarity) -> SlotAttributionEvidence {
    SlotAttributionEvidence {
        schema_version: SLOT_ATTRIBUTION_SCHEMA_VERSION,
        slot_id: "edge_slot".to_string(),
        embedder: Some(EmbedderId("E_EDGE".to_string())),
        chunk: Some(chunk()),
        polarity,
        source: SlotAttributionSource::VerdictHead,
        score: 0.8,
        threshold: None,
        margin: None,
        reason: "edge-case attribution".to_string(),
        relationship_slot_id: None,
        related_fingerprint_id: None,
        active_learning_candidate_id: None,
        q_head: Some("q2_verdict".to_string()),
        impact_kind: None,
        evidence: "edge-case evidence".to_string(),
    }
}

fn count_polarity(prediction: &RealityPrediction, polarity: SlotAttributionPolarity) -> usize {
    prediction
        .slot_attributions
        .iter()
        .filter(|row| row.polarity == polarity)
        .count()
}

fn count_source(prediction: &RealityPrediction, source: SlotAttributionSource) -> usize {
    prediction
        .slot_attributions
        .iter()
        .filter(|row| row.source == source)
        .count()
}

fn deterministic_panel() -> Result<Panel> {
    let data = (0..PANEL_DIM)
        .map(|idx| ((idx % 29) as f32) * 0.001)
        .collect::<Vec<_>>();
    let filled_mask = (1u16 << InstrumentSlot::all().len()) - 1;
    Ok(Panel::try_new(data, filled_mask)?)
}

fn chunk() -> ChunkId {
    ChunkId("src/user.py#function:normalize_user#0".to_string())
}

fn count_physical_cf(db: &DB, cf_name: &str) -> Result<u64> {
    let cf = db
        .cf_handle(cf_name)
        .with_context(|| format!("missing column family {cf_name}"))?;
    Ok(db.iterator_cf(cf, IteratorMode::Start).count() as u64)
}

fn sha256_file(path: &Path) -> Result<String> {
    let bytes = fs::read(path)?;
    Ok(hex::encode(Sha256::digest(&bytes)))
}
