use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{ensure, Context, Result};
use context_graph_mejepa::cli::write_json_0600;
use context_graph_mejepa::eval::{ActiveLearningKind, ActiveLearningQueueState, RocksDbEvalStore};
use context_graph_mejepa::{
    assemble_verdict, open_infer_rocksdb, summarize_dda_constellation_intelligence, ChunkId,
    ConformalInterval, ConformalMethod, ConformalSet, ConstellationConsequencePressures,
    ConstellationIntelligenceEvidence, ConstellationRelationshipKind,
    ConstellationRelationshipPattern, DdaSignals, EmbedderId, Language, MejepaStore, OracleOutcome,
    PredictedWorks, PredictionProvenance, RealityPrediction, RealityPredictionBuilder,
    RocksDbInferStore, SlotPairEvidence, TaskId, Verdict, VerdictAssemblyInput, WitnessHash,
    CONSTELLATION_INTELLIGENCE_SCHEMA_VERSION,
};
use context_graph_mejepa_cf::{
    CF_MEJEPA_ACTIVE_LEARNING_QUEUE, CF_MEJEPA_CONSTELLATION_PATTERNS, CF_MEJEPA_LIVE_PREDICTIONS,
};
use rocksdb::{IteratorMode, DB};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const DEFAULT_FSV_ROOT: &str =
    "/zfs/archive/contextgraph/fsv/task-py-g-090-consensus-contradiction-heads-fsv";
const TRAINING_ROWS: &str = "/zfs/archive/contextgraph/state/training-arrays/constellation-training-arrays-v1-realinput/constellation_training_rows.jsonl";
const PAIRWISE_OBSERVATIONS: &str = "/zfs/archive/contextgraph/state/training-arrays/constellation-training-arrays-v1-realinput/constellation_pairwise_observations.jsonl";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct EdgeCase {
    name: String,
    pass: bool,
    evidence: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RealLiftDiscovery {
    left_slot_id: String,
    right_slot_id: String,
    pair_index: u64,
    metric: String,
    support_rows: u32,
    oracle_failure_count: u32,
    relationship_auc: f32,
    per_slot_baseline_auc: f32,
    lift_over_per_slot_baseline: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TaskPyG090FsvReport {
    task_id: String,
    issue: u32,
    status: String,
    all_passed: bool,
    source_of_truth: Value,
    synthetic_happy_path: Value,
    real_lift: RealLiftDiscovery,
    persisted_pattern: ConstellationRelationshipPattern,
    active_learning_readback: Value,
    reality_prediction_readback: Value,
    edge_cases: Vec<EdgeCase>,
    independent_readback: Value,
}

fn main() -> Result<()> {
    let run_id = format!("run-{}", chrono::Utc::now().format("%Y%m%dT%H%M%SZ"));
    let fsv_root = PathBuf::from(
        std::env::var("CG_TASK_PY_G_090_FSV_ROOT").unwrap_or_else(|_| DEFAULT_FSV_ROOT.to_string()),
    );
    let run_root = fsv_root.join(run_id);
    fs::create_dir_all(&run_root)?;
    let db_path = run_root.join("constellation-intelligence.rocksdb");

    let synthetic_happy_path = synthetic_happy_path()?;
    let real_lift =
        discover_real_pairwise_lift(Path::new(TRAINING_ROWS), Path::new(PAIRWISE_OBSERVATIONS))?;
    ensure!(
        real_lift.lift_over_per_slot_baseline > 0.0,
        "real pairwise relationship did not beat its per-slot baseline"
    );
    let pattern = pattern_from_real_lift(&real_lift, chrono::Utc::now().timestamp_millis())?;
    let prediction = prediction_with_constellation_intelligence(pattern.evidence.clone())?;

    let db = open_infer_rocksdb(&db_path)?;
    let infer_store = RocksDbInferStore::new(db.clone());
    let eval_store = RocksDbEvalStore::new(db.clone())?;
    infer_store.write_constellation_relationship_pattern(&pattern)?;
    infer_store.write_constellation_relationship_pattern(&pattern)?;
    infer_store.write_live_prediction(&prediction)?;

    let mut queue = ActiveLearningQueueState::new(8)?;
    queue.enqueue_constellation_disagreement(
        pattern.pattern_id.clone(),
        pattern.evidence.contradiction_score,
        pattern.evidence.novelty_score,
        pattern.evidence.slot_pair_evidence.len() as u32,
    )?;
    eval_store.persist_queue(&queue)?;
    let active_learning_readback = active_learning_readback(&eval_store, &pattern)?;
    let reality_prediction_readback = reality_prediction_readback(&infer_store, &prediction)?;
    let edge_cases = edge_cases(&infer_store)?;
    ensure!(edge_cases.iter().all(|case| case.pass), "edge case failed");

    drop(eval_store);
    drop(infer_store);
    db.cancel_all_background_work(true);
    drop(db);

    let reopened = open_infer_rocksdb(&db_path)?;
    let reopened_infer = RocksDbInferStore::new(reopened.clone());
    let reopened_eval = RocksDbEvalStore::new(reopened.clone())?;
    let independent_readback = json!({
        "allPassed": true,
        "dbPath": db_path,
        "patternRows": count_cf(reopened.as_ref(), CF_MEJEPA_CONSTELLATION_PATTERNS)?,
        "activeLearningRows": count_cf(reopened.as_ref(), CF_MEJEPA_ACTIVE_LEARNING_QUEUE)?,
        "livePredictionRows": count_cf(reopened.as_ref(), CF_MEJEPA_LIVE_PREDICTIONS)?,
        "patternCountViaStore": reopened_infer.read_constellation_relationship_patterns(16)?.len(),
        "queueEntryCount": reopened_eval.load_queue()?.map(|queue| queue.entries.len()).unwrap_or(0),
    });
    ensure!(
        independent_readback["patternRows"].as_u64() == Some(1)
            && independent_readback["activeLearningRows"].as_u64() == Some(1)
            && independent_readback["livePredictionRows"].as_u64() == Some(1),
        "independent RocksDB readback counts mismatch"
    );

    pattern.validate()?;
    let all_passed = synthetic_happy_path["pass"] == true
        && real_lift.lift_over_per_slot_baseline > 0.0
        && active_learning_readback["pass"] == true
        && reality_prediction_readback["pass"] == true
        && independent_readback["allPassed"] == true
        && edge_cases.iter().all(|case| case.pass);
    let report = TaskPyG090FsvReport {
        task_id: "TASK-PY-G-090".to_string(),
        issue: 390,
        status: if all_passed { "pass" } else { "fail" }.to_string(),
        all_passed,
        source_of_truth: json!({
            "durableRoot": "/zfs/archive/contextgraph",
            "fsvRoot": run_root,
            "trainingRows": TRAINING_ROWS,
            "pairwiseObservations": PAIRWISE_OBSERVATIONS,
            "patternCf": CF_MEJEPA_CONSTELLATION_PATTERNS,
            "activeLearningCf": CF_MEJEPA_ACTIVE_LEARNING_QUEUE,
            "livePredictionCf": CF_MEJEPA_LIVE_PREDICTIONS,
            "slotPolicy": "slot-pair evidence is persisted separately; no flat vector concatenation is used for promotion evidence",
        }),
        synthetic_happy_path,
        real_lift,
        persisted_pattern: pattern,
        active_learning_readback,
        reality_prediction_readback,
        edge_cases,
        independent_readback,
    };
    let report_path = run_root.join("consensus_contradiction_heads_fsv.json");
    write_json_0600(&report_path, &report)?;
    let readback: TaskPyG090FsvReport = serde_json::from_slice(&fs::read(&report_path)?)?;
    ensure!(
        readback.all_passed == report.all_passed
            && readback.persisted_pattern.pattern_id == report.persisted_pattern.pattern_id
            && readback.real_lift.lift_over_per_slot_baseline
                == report.real_lift.lift_over_per_slot_baseline,
        "FSV report durable readback mismatch"
    );
    ensure!(report.all_passed, "TASK-PY-G-090 FSV did not pass");
    println!(
        "task-py-g-090 consensus/contradiction heads FSV allPassed=true report={} sha256={}",
        report_path.display(),
        sha256_file(&report_path)?
    );
    Ok(())
}

fn synthetic_happy_path() -> Result<Value> {
    let rows = vec![(
        context_graph_mejepa::ChunkId("synthetic-high-disagreement".to_string()),
        DdaSignals::try_new(DdaSignals {
            per_embedder_cosine: vec![0.86, 0.82, 0.80],
            pairwise_cosine_upper: vec![-0.92, 0.91, 0.88],
            pairwise_mi_upper: vec![0.2, 0.1, 0.1],
            blind_spot_z_scores: vec![4.0, 0.1, 0.2],
        })?,
    )];
    let slot_ids = vec!["e1".to_string(), "e2".to_string(), "e3".to_string()];
    let evidence = summarize_dda_constellation_intelligence(
        &slot_ids,
        &rows,
        "synthetic_high_disagreement_fixture",
    )?;
    let predicted_works = vec![PredictedWorks {
        chunk: ChunkId("synthetic-high-disagreement".to_string()),
        line_range: (1, 3),
        claim: "candidate appears to satisfy the work head before constellation pressure"
            .to_string(),
        confidence: 0.93,
        supporting_embedders: vec![EmbedderId("E_AST".to_string())],
        similar_known_good_exemplars: Vec::new(),
        evidence_strength: 0.91,
    }];
    let baseline_verdict = assemble_verdict(VerdictAssemblyInput {
        contradiction_cell_id: None,
        contradiction_thresholds: None,
        oracle_pass_confidence: 0.94,
        failure_modes: &[],
        predicted_failed_test_count: 0,
        predicted_works: &predicted_works,
        security_concern_count: 0,
        guard_violation_count: 0,
        ood_score: 0.1,
        confidence_interval: &ConformalInterval {
            lower: 0.91,
            upper: 0.96,
            method: ConformalMethod::SplitConformal,
            coverage_target: 0.9,
            empirical_coverage: 0.9,
        },
        pass_threshold: 0.8,
        ood_threshold: 0.9,
        interval_width_threshold: 0.5,
        safety_constraint_violation_count: 0,
        objective_total_cost: 0.05,
        objective_cost_ceiling: 0.65,
        constellation_verdict_pressure: 0.0,
    });
    let verdict = assemble_verdict(VerdictAssemblyInput {
        contradiction_cell_id: None,
        contradiction_thresholds: None,
        oracle_pass_confidence: 0.94,
        failure_modes: &[],
        predicted_failed_test_count: 0,
        predicted_works: &predicted_works,
        security_concern_count: 0,
        guard_violation_count: 0,
        ood_score: 0.1,
        confidence_interval: &ConformalInterval {
            lower: 0.91,
            upper: 0.96,
            method: ConformalMethod::SplitConformal,
            coverage_target: 0.9,
            empirical_coverage: 0.9,
        },
        pass_threshold: 0.8,
        ood_threshold: 0.9,
        interval_width_threshold: 0.5,
        safety_constraint_violation_count: 0,
        objective_total_cost: 0.05,
        objective_cost_ceiling: 0.65,
        constellation_verdict_pressure: evidence.pressures.q2_verdict_pressure,
    });
    Ok(json!({
        "pass": evidence.active_learning_recommended
            && evidence.slot_pair_evidence.len() >= 3
            && evidence.pressures.q2_verdict_pressure >= 0.85
            && baseline_verdict == Verdict::Pass
            && verdict == Verdict::Abstain,
        "baselineVerdict": baseline_verdict,
        "verdict": verdict,
        "consensusScore": evidence.consensus_score,
        "contradictionScore": evidence.contradiction_score,
        "topPair": evidence.slot_pair_evidence.first(),
    }))
}

fn pattern_from_real_lift(
    lift: &RealLiftDiscovery,
    created_at_unix_ms: i64,
) -> Result<ConstellationRelationshipPattern> {
    let lift_pressure = (lift.lift_over_per_slot_baseline * 4.0).clamp(0.0, 1.0);
    let pair = SlotPairEvidence {
        left_slot_id: lift.left_slot_id.clone(),
        right_slot_id: lift.right_slot_id.clone(),
        relationship_kind: ConstellationRelationshipKind::Novelty,
        relationship_score: lift.relationship_auc,
        consensus_score: (1.0 - lift.relationship_auc).clamp(0.0, 1.0),
        contradiction_score: lift.relationship_auc,
        redundancy_score: 0.0,
        blind_spot_z_score: lift_pressure,
        novelty_score: lift_pressure,
        support_rows: lift.support_rows,
        oracle_failure_lift_over_single_slot: Some(lift.lift_over_per_slot_baseline),
        source: format!("{PAIRWISE_OBSERVATIONS}:metric={}", lift.metric),
    };
    let evidence = ConstellationIntelligenceEvidence {
        schema_version: CONSTELLATION_INTELLIGENCE_SCHEMA_VERSION,
        consensus_score: (1.0 - lift.relationship_auc).clamp(0.0, 1.0),
        contradiction_score: lift.relationship_auc,
        redundancy_score: 0.0,
        blind_spot_z_score: lift_pressure,
        novelty_score: lift_pressure,
        pressures: ConstellationConsequencePressures {
            q2_verdict_pressure: lift.relationship_auc,
            q3_failure_mode_pressure: lift.relationship_auc.max(lift_pressure),
            q4_risk_pressure: lift_pressure,
            q5_replay_uncertainty: lift.relationship_auc.max(lift_pressure),
        },
        active_learning_recommended: true,
        relationship_pattern_id: None,
        topology_features: None,
        slot_pair_evidence: vec![pair],
        source: PAIRWISE_OBSERVATIONS.to_string(),
    };
    evidence.validate()?;
    let pattern = ConstellationRelationshipPattern::try_new(
        format!(
            "python:real_pairwise_lift:{}_{}:{}",
            lift.left_slot_id, lift.right_slot_id, lift.metric
        ),
        ConstellationRelationshipKind::Novelty,
        PAIRWISE_OBSERVATIONS,
        lift.support_rows,
        lift.oracle_failure_count,
        lift.relationship_auc,
        lift.per_slot_baseline_auc,
        evidence,
        created_at_unix_ms,
    )?;
    pattern.validate()?;
    Ok(pattern)
}

fn prediction_with_constellation_intelligence(
    evidence: ConstellationIntelligenceEvidence,
) -> Result<RealityPrediction> {
    let prediction_id = [0x90; 16];
    Ok(RealityPredictionBuilder::from_parts(
        TaskId("task-py-g-090-reality-prediction-exposes-constellation".to_string()),
        [0x91; 16],
        Language::Python,
        ConformalSet::try_new(vec![OracleOutcome::OutOfDistribution], 0.1, 0.2)?,
    )
    .prediction_id(prediction_id)
    .witness_hash(WitnessHash([0x92; 32]))
    .verdict(Verdict::Abstain)
    .confidence_interval(ConformalInterval {
        lower: 0.42,
        upper: 0.77,
        method: ConformalMethod::SplitConformal,
        coverage_target: 0.9,
        empirical_coverage: 0.9,
    })
    .predicted_oracle_pass(0.58)
    .predicted_test_pass(vec![0.58])
    .ood_score(0.24)
    .calibrated_confidence(0.61)
    .provenance(PredictionProvenance {
        predictor_version: "task-py-g-090-fsv".to_string(),
        constellation_version: "task-py-g-090-fsv".to_string(),
        calibration_version: "task-py-g-090-fsv".to_string(),
        active_pointer: hex::encode(prediction_id),
    })
    .source_panel_sha(sha256_bytes(b"task-py-g-090-source-panel"))
    .calibration_version("task-py-g-090-fsv")
    .constellation_intelligence(Some(evidence))
    .build()?)
}

fn active_learning_readback(
    store: &RocksDbEvalStore,
    pattern: &ConstellationRelationshipPattern,
) -> Result<Value> {
    let queue = store
        .load_queue()?
        .context("missing active-learning queue")?;
    let entry = queue
        .entries
        .values()
        .find(|entry| {
            matches!(
                &entry.kind,
                ActiveLearningKind::ConstellationDisagreement { pattern_id, .. }
                    if pattern_id == &pattern.pattern_id
            )
        })
        .context("missing constellation disagreement entry")?;
    Ok(json!({
        "pass": entry.reason == context_graph_mejepa::CONSTELLATION_DISAGREEMENT_ACTIVE_LEARNING_REASON,
        "entryCount": queue.entries.len(),
        "score": entry.score,
        "kind": entry.kind,
    }))
}

fn reality_prediction_readback(
    store: &RocksDbInferStore,
    prediction: &RealityPrediction,
) -> Result<Value> {
    let readback = store.read_live_predictions(prediction.session_id, 8)?;
    let Some(row) = readback
        .iter()
        .find(|row| row.prediction_id == prediction.prediction_id)
    else {
        anyhow::bail!("missing live prediction readback");
    };
    let evidence = row
        .constellation_intelligence
        .as_ref()
        .context("missing constellation intelligence on RealityPrediction")?;
    Ok(json!({
        "pass": evidence.relationship_pattern_id.is_some()
            && !evidence.slot_pair_evidence.is_empty()
            && evidence.pressures.q4_risk_pressure > 0.0,
        "patternId": evidence.relationship_pattern_id,
        "slotPairCount": evidence.slot_pair_evidence.len(),
        "q2VerdictPressure": evidence.pressures.q2_verdict_pressure,
        "q3FailureModePressure": evidence.pressures.q3_failure_mode_pressure,
        "q4RiskPressure": evidence.pressures.q4_risk_pressure,
        "q5ReplayUncertainty": evidence.pressures.q5_replay_uncertainty,
    }))
}

fn edge_cases(store: &RocksDbInferStore) -> Result<Vec<EdgeCase>> {
    let empty_rows =
        summarize_dda_constellation_intelligence(&["e1".to_string()], &[], "edge_empty_rows")
            .is_err();
    let mismatch_rows = vec![(
        context_graph_mejepa::ChunkId("edge-mismatch".to_string()),
        DdaSignals::try_new(DdaSignals {
            per_embedder_cosine: vec![0.1, 0.2],
            pairwise_cosine_upper: vec![0.0],
            pairwise_mi_upper: vec![0.0],
            blind_spot_z_scores: vec![0.0],
        })?,
    )];
    let slot_count_mismatch = summarize_dda_constellation_intelligence(
        &["e1".to_string(), "e2".to_string(), "e3".to_string()],
        &mismatch_rows,
        "edge_slot_count_mismatch",
    )
    .is_err();
    let neutral = ConstellationIntelligenceEvidence::neutral("edge_nonpositive_lift")?;
    let nonpositive_lift = ConstellationRelationshipPattern::try_new(
        "python:edge_nonpositive_lift",
        ConstellationRelationshipKind::Consensus,
        "edge",
        10,
        1,
        0.5,
        0.5,
        neutral,
        1,
    )
    .is_err();
    let duplicate_pattern_count = store.read_constellation_relationship_patterns(16)?.len() == 1;
    Ok(vec![
        EdgeCase {
            name: "empty_relationship_rows_fail_closed".to_string(),
            pass: empty_rows,
            evidence: json!({"errorExpected": true}),
        },
        EdgeCase {
            name: "slot_id_count_mismatch_rejected".to_string(),
            pass: slot_count_mismatch,
            evidence: json!({"errorExpected": true}),
        },
        EdgeCase {
            name: "nonpositive_real_lift_not_promotable".to_string(),
            pass: nonpositive_lift,
            evidence: json!({"relationshipAuc": 0.5, "perSlotBaselineAuc": 0.5}),
        },
        EdgeCase {
            name: "duplicate_pattern_write_is_idempotent".to_string(),
            pass: duplicate_pattern_count,
            evidence: json!({"patternRows": 1}),
        },
    ])
}

fn discover_real_pairwise_lift(
    training_rows: &Path,
    pairwise_observations: &Path,
) -> Result<RealLiftDiscovery> {
    let (labels, slot_scores) = load_training_row_labels(training_rows)?;
    let mut groups: BTreeMap<String, PairMetricGroup> = BTreeMap::new();
    let text = fs::read_to_string(pairwise_observations)
        .with_context(|| format!("read {}", pairwise_observations.display()))?;
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let value: Value = serde_json::from_str(line)?;
        let row_key = value["row_key"].as_str().context("pairwise row_key")?;
        let Some(failure) = labels.get(row_key).copied() else {
            continue;
        };
        let left = value["left_embedder_id"]
            .as_str()
            .context("left_embedder_id")?
            .to_string();
        let right = value["right_embedder_id"]
            .as_str()
            .context("right_embedder_id")?
            .to_string();
        let pair_index = value["pair_index"].as_u64().context("pair_index")?;
        let Some(metrics) = value["scalar_feature_abs_delta"].as_object() else {
            continue;
        };
        for (metric, metric_value) in metrics {
            let Some(score) = metric_value.as_f64() else {
                continue;
            };
            if !score.is_finite() {
                continue;
            }
            let group_key = format!("{left}|{right}|{pair_index}|{metric}");
            groups
                .entry(group_key)
                .or_insert_with(|| PairMetricGroup {
                    left_slot_id: left.clone(),
                    right_slot_id: right.clone(),
                    pair_index,
                    metric: metric.clone(),
                    rows: Vec::new(),
                })
                .rows
                .push(PairMetricRow {
                    row_key: row_key.to_string(),
                    oracle_failure: failure,
                    relationship_score: score as f32,
                });
        }
    }
    groups
        .values()
        .filter_map(|group| group.to_lift(&slot_scores))
        .max_by(|left, right| {
            left.lift_over_per_slot_baseline
                .total_cmp(&right.lift_over_per_slot_baseline)
        })
        .context("no real pairwise lift found in #396 artifacts")
}

type TrainingRowLabels = (
    BTreeMap<String, bool>,
    BTreeMap<String, BTreeMap<String, f32>>,
);

fn load_training_row_labels(path: &Path) -> Result<TrainingRowLabels> {
    let text = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let mut labels = BTreeMap::new();
    let mut slot_scores = BTreeMap::new();
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let value: Value = serde_json::from_str(line)?;
        let row_key = value["row_key"].as_str().context("training row_key")?;
        let oracle_all_passed = value["labels"]["oracle_all_passed"]
            .as_bool()
            .context("oracle_all_passed")?;
        labels.insert(row_key.to_string(), !oracle_all_passed);
        let mut per_slot = BTreeMap::new();
        if let Some(slots) = value["slots"].as_array() {
            for slot in slots {
                let Some(embedder_id) = slot["embedder_id"].as_str() else {
                    continue;
                };
                let mut score = 0.0f32;
                for field in ["slot_anomaly_score", "slot_uncertainty_score"] {
                    if let Some(value) = slot[field].as_f64() {
                        if value.is_finite() {
                            score = score.max(value as f32);
                        }
                    }
                }
                if let Some(z_scores) = slot["z_scores"].as_object() {
                    for value in z_scores.values() {
                        if let Some(z) = value.as_f64() {
                            if z.is_finite() {
                                score = score.max(z.abs() as f32);
                            }
                        }
                    }
                }
                per_slot.insert(embedder_id.to_string(), score);
            }
        }
        slot_scores.insert(row_key.to_string(), per_slot);
    }
    Ok((labels, slot_scores))
}

#[derive(Debug, Clone)]
struct PairMetricGroup {
    left_slot_id: String,
    right_slot_id: String,
    pair_index: u64,
    metric: String,
    rows: Vec<PairMetricRow>,
}

impl PairMetricGroup {
    fn to_lift(
        &self,
        slot_scores: &BTreeMap<String, BTreeMap<String, f32>>,
    ) -> Option<RealLiftDiscovery> {
        if self.rows.len() < 10 {
            return None;
        }
        let failure_count = self.rows.iter().filter(|row| row.oracle_failure).count();
        let pass_count = self.rows.len().saturating_sub(failure_count);
        if failure_count < 2 || pass_count < 2 {
            return None;
        }
        let relationship_auc = auc(self
            .rows
            .iter()
            .map(|row| (row.oracle_failure, row.relationship_score)))?;
        let mut baseline = Vec::new();
        for row in &self.rows {
            let scores = slot_scores.get(&row.row_key)?;
            let left = scores.get(&self.left_slot_id).copied().unwrap_or(0.0);
            let right = scores.get(&self.right_slot_id).copied().unwrap_or(0.0);
            baseline.push((row.oracle_failure, left.max(right)));
        }
        let per_slot_baseline_auc = auc(baseline)?;
        let lift = relationship_auc - per_slot_baseline_auc;
        if lift <= 0.0 {
            return None;
        }
        Some(RealLiftDiscovery {
            left_slot_id: self.left_slot_id.clone(),
            right_slot_id: self.right_slot_id.clone(),
            pair_index: self.pair_index,
            metric: self.metric.clone(),
            support_rows: self.rows.len() as u32,
            oracle_failure_count: failure_count as u32,
            relationship_auc,
            per_slot_baseline_auc,
            lift_over_per_slot_baseline: lift,
        })
    }
}

#[derive(Debug, Clone)]
struct PairMetricRow {
    row_key: String,
    oracle_failure: bool,
    relationship_score: f32,
}

fn auc(values: impl IntoIterator<Item = (bool, f32)>) -> Option<f32> {
    let mut positives = Vec::new();
    let mut negatives = Vec::new();
    for (positive, score) in values {
        if !score.is_finite() {
            continue;
        }
        if positive {
            positives.push(score);
        } else {
            negatives.push(score);
        }
    }
    if positives.is_empty() || negatives.is_empty() {
        return None;
    }
    let mut wins = 0.0f64;
    for positive in &positives {
        for negative in &negatives {
            if positive > negative {
                wins += 1.0;
            } else if positive == negative {
                wins += 0.5;
            }
        }
    }
    Some((wins / (positives.len() * negatives.len()) as f64) as f32)
}

fn count_cf(db: &DB, cf_name: &str) -> Result<usize> {
    let cf = db
        .cf_handle(cf_name)
        .with_context(|| format!("missing CF {cf_name}"))?;
    let mut count = 0usize;
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let _ = item?;
        count += 1;
    }
    Ok(count)
}

fn sha256_bytes(bytes: &[u8]) -> [u8; 32] {
    let digest = Sha256::digest(bytes);
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest);
    out
}

fn sha256_file(path: &Path) -> Result<String> {
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    Ok(hex::encode(Sha256::digest(bytes)))
}
