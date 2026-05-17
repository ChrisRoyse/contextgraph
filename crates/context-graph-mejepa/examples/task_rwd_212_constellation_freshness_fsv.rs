use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use anyhow::{Context, Result};
use context_graph_mejepa::heal::{SchedulerState, SelfOptimConfig, TickName};
use context_graph_mejepa::types::{
    AgentClaimGraph, ClaimGraph, ConformalInterval, ConformalSet, Language as InferLanguage,
    OracleOutcome as InferOracleOutcome, PredictionProvenance, RealityPrediction, ReasoningClass,
    SkillCitation, TaskContext, TaskEnvironment, TaskId, TestId, Verdict, WitnessHash,
};
use context_graph_mejepa::{ConstellationGuard, TctConstellationGuard};
use context_graph_mejepa_instruments::{InstrumentSlot, Panel, PANEL_DIM};
use context_graph_mejepa_tct::{
    build_freshness_audit, freshness_bypass_entry, materialize_constellation_cells,
    overrides_from_refresh_log, should_refresh, Centroid, ConstellationRefreshLogEntry,
    ConstellationRefreshLogEntryInput, ConstellationStore, CorpusProvenance, EmbedderId,
    EntityType, Language, MutationCategory, OracleOutcome, RefreshActionStatus, RefreshDecision,
    RefreshPolicyConfig, RefreshReason, ShrinkageOrigin, TctConstellation, Thresholds,
};
use rocksdb::{ColumnFamilyDescriptor, Options, DB};
use serde_json::json;

const FSV_ROOT: &str = "/mnt/d/contextgraph/fsv/task-py-g-052-constellation-freshness-fsv";

fn main() -> Result<()> {
    let run_root =
        PathBuf::from(FSV_ROOT).join(format!("run-{}", chrono::Utc::now().timestamp_millis()));
    fs::create_dir_all(&run_root)?;

    let happy = scheduler_refits_and_advances_cells(&run_root.join("happy"))?;
    let failed = scheduler_refit_failure_is_logged(&run_root.join("failed"))?;
    let skip = all_policy_clauses_skip_without_log(&run_root.join("skip"))?;
    let policy = policy_edges_and_operator_override(&run_root.join("policy"))?;
    let inference = stale_inference_cell_fails_closed()?;

    let cases = vec![
        happy.clone(),
        failed.clone(),
        skip.clone(),
        policy.clone(),
        inference.clone(),
    ];
    let all_passed = cases
        .iter()
        .all(|case| case["pass"].as_bool().unwrap_or(false));
    let artifact = json!({
        "task": "TASK-RWD-212 / TASK-PY-G-052",
        "issue": 271,
        "all_passed": all_passed,
        "cases": cases,
        "source_of_truth": {
            "constellation_cf": context_graph_mejepa_cf::CF_MEJEPA_CONSTELLATION,
            "refresh_log_cf": context_graph_mejepa_cf::CF_MEJEPA_CONSTELLATION_REFRESH_LOG,
            "refresh_report_cf": context_graph_mejepa_cf::CF_MEJEPA_CONSTELLATION_REFRESH_REPORTS,
            "fsv_root": FSV_ROOT,
        }
    });
    let artifact_path = run_root.join("constellation_freshness_fsv.json");
    write_json_0600(&artifact_path, &artifact)?;
    println!("{}", serde_json::to_string_pretty(&artifact)?);
    anyhow::ensure!(all_passed, "constellation freshness FSV assertions failed");
    Ok(())
}

fn scheduler_refits_and_advances_cells(root: &Path) -> Result<serde_json::Value> {
    fs::create_dir_all(root)?;
    let db_path = root.join("db");
    let db = open_runtime_rocksdb(&db_path)?;
    let store = ConstellationStore::new(db.clone())?;
    let constellation = fixture_constellation(context_graph_mejepa_tct::days_ago(91))?;
    let version = store.persist(&constellation)?;
    write_train_cert_rows(db.as_ref(), 101)?;

    let config = scheduler_config(root);
    let mut state = SchedulerState::open(db.clone(), config)?;
    state.run_tick(TickName::ConstellationFreshness)?;

    let entries = store.load_refresh_log_entries(10_000)?;
    let success_count = entries
        .iter()
        .filter(|entry| entry.status == RefreshActionStatus::RefitSucceeded)
        .count();
    let overrides = overrides_from_refresh_log(version, &entries);
    let cells = materialize_constellation_cells(&constellation, &overrides)?;
    let advanced = cells
        .iter()
        .all(|cell| cell.last_refresh_ts > constellation.frozen_at);
    let shrunk_logged = entries.iter().any(|entry| entry.bayesian_shrunk);

    Ok(json!({
        "case": "scheduler_refits_and_advances_cells",
        "pass": success_count == constellation.per_chunk_type_centroids.len() && advanced && shrunk_logged,
        "refresh_log_count": store.count_refresh_log_entries()?,
        "success_count": success_count,
        "advanced_all_cells": advanced,
        "shrunk_cell_refresh_logged": shrunk_logged,
    }))
}

fn scheduler_refit_failure_is_logged(root: &Path) -> Result<serde_json::Value> {
    fs::create_dir_all(root)?;
    let db_path = root.join("db");
    let db = open_runtime_rocksdb(&db_path)?;
    let store = ConstellationStore::new(db.clone())?;
    let constellation = fixture_constellation(context_graph_mejepa_tct::days_ago(91))?;
    store.persist(&constellation)?;

    let config = scheduler_config(root);
    let mut state = SchedulerState::open(db, config)?;
    state.run_tick(TickName::ConstellationFreshness)?;

    let entries = store.load_refresh_log_entries(10_000)?;
    let failed_count = entries
        .iter()
        .filter(|entry| entry.status == RefreshActionStatus::RefreshFailed)
        .count();
    Ok(json!({
        "case": "scheduler_refit_failure_is_logged",
        "pass": failed_count == constellation.per_chunk_type_centroids.len(),
        "failed_count": failed_count,
        "expected": "REFRESH_FAILED for every stale cell when corpus/train-cert source rows are unavailable",
    }))
}

fn all_policy_clauses_skip_without_log(root: &Path) -> Result<serde_json::Value> {
    fs::create_dir_all(root)?;
    let db_path = root.join("db");
    let db = open_runtime_rocksdb(&db_path)?;
    let store = ConstellationStore::new(db.clone())?;
    let constellation = fixture_constellation(SystemTime::now() - Duration::from_secs(10))?;
    store.persist(&constellation)?;

    let config = scheduler_config(root);
    let mut state = SchedulerState::open(db, config)?;
    state.run_tick(TickName::ConstellationFreshness)?;

    Ok(json!({
        "case": "all_policy_clauses_skip_without_log",
        "pass": store.count_refresh_log_entries()? == 0,
        "refresh_log_count": store.count_refresh_log_entries()?,
    }))
}

fn policy_edges_and_operator_override(root: &Path) -> Result<serde_json::Value> {
    fs::create_dir_all(root)?;
    let db = open_runtime_rocksdb(&root.join("db"))?;
    let store = ConstellationStore::new(db.clone())?;
    let constellation = fixture_constellation(SystemTime::now() - Duration::from_secs(10))?;
    let version = store.persist(&constellation)?;
    let cells = materialize_constellation_cells(&constellation, &BTreeMap::new())?;
    let mut examples_cell = cells[0];
    examples_cell.examples_since_last_refresh = 101;
    let mut drift_cell = cells[1];
    drift_cell.tau_m_drift_pct_rolling = 3.1;
    let mut skip_cell = cells[2];
    skip_cell.examples_since_last_refresh = 100;
    let now = SystemTime::now();
    let config = RefreshPolicyConfig::default();

    let examples_decision = should_refresh(&examples_cell, now, config)?;
    let drift_decision = should_refresh(&drift_cell, now, config)?;
    let skip_decision = should_refresh(&skip_cell, now, config)?;

    let skip_entry = ConstellationRefreshLogEntry::try_new(ConstellationRefreshLogEntryInput {
        constellation_version_id: version,
        cell: skip_cell,
        decision: skip_decision.clone(),
        status: RefreshActionStatus::Skipped,
        generated_at: now,
        after_last_refresh_ts: None,
        operator_id: None,
        detail: "SKIPPED: all refresh policy clauses below threshold".to_string(),
    })?;
    store.persist_refresh_log_entry(&skip_entry)?;
    let override_entry = freshness_bypass_entry(
        version,
        examples_cell,
        now,
        "operator-fsv",
        "MANUAL_OVERRIDE_FRESHNESS_BYPASS during FSV",
    )?;
    store.persist_refresh_log_entry(&override_entry)?;
    drop(store);
    drop(db);
    let reopened = open_runtime_rocksdb(&root.join("db"))?;
    let reopened_store = ConstellationStore::new(reopened)?;
    let readback = reopened_store.load_refresh_log_entries(10)?;
    let audit = build_freshness_audit(
        version,
        &[examples_cell, drift_cell, skip_cell],
        now,
        config,
    )?;

    Ok(json!({
        "case": "policy_edges_and_operator_override",
        "pass": matches!(examples_decision, RefreshDecision::Refit { reason: RefreshReason::NewExamples { .. }, .. })
            && matches!(drift_decision, RefreshDecision::Refit { reason: RefreshReason::TauMDrift { .. }, .. })
            && matches!(skip_decision, RefreshDecision::Skip { .. })
            && readback.iter().any(|entry| entry.status == RefreshActionStatus::ManualOverrideFreshnessBypass)
            && audit.refit_required_count == 2
            && audit.skip_count == 1,
        "audit": {
            "refit_required_count": audit.refit_required_count,
            "skip_count": audit.skip_count,
            "shrunk_refit_count": audit.shrunk_refit_count,
        },
        "refresh_log_count_after_reopen": readback.len(),
    }))
}

fn stale_inference_cell_fails_closed() -> Result<serde_json::Value> {
    let constellation = fixture_constellation(context_graph_mejepa_tct::days_ago(91))?;
    let guard = TctConstellationGuard::new(
        constellation,
        MutationCategory::KnownGood,
        EntityType::Function,
    )?;
    let prediction = test_prediction();
    let panel = dummy_panel()?;
    let context = test_context();
    let err = guard
        .verify(&prediction, &panel, &context)
        .expect_err("stale cell must fail closed");
    Ok(json!({
        "case": "stale_inference_cell_fails_closed",
        "pass": err.code() == "MEJEPA_INFER_GTAU_STALE_CONSTELLATION",
        "error_code": err.code(),
        "error": err.to_string(),
    }))
}

fn fixture_constellation(frozen_at: SystemTime) -> Result<TctConstellation> {
    let mut thresholds = BTreeMap::new();
    let mut per_chunk_thresholds = BTreeMap::new();
    let mut category = BTreeMap::new();
    let mut language = BTreeMap::new();
    let mut outcome = BTreeMap::new();
    let mut chunk = BTreeMap::new();
    for (idx, embedder) in EmbedderId::all().iter().copied().enumerate() {
        thresholds.insert(embedder, 0.5);
        per_chunk_thresholds.insert((EntityType::Function, embedder), 0.5);
        let sample_count = if idx == 0 { 25 } else { 80 };
        let centroid = Centroid::try_new(
            normalized_fixture_vector(8, idx as f32 + 1.0),
            sample_count,
            ShrinkageOrigin::OwnCell,
            "fsv-centroid",
        )?;
        category.insert(embedder, centroid.clone());
        language.insert(embedder, centroid.clone());
        outcome.insert(embedder, centroid.clone());
        chunk.insert(
            (
                MutationCategory::KnownGood,
                EntityType::Function,
                Language::Python,
                embedder,
            ),
            centroid,
        );
    }
    let thresholds = Thresholds::try_new(thresholds, per_chunk_thresholds)?;
    let provenance = CorpusProvenance::try_new(
        [0x31; 32],
        EmbedderId::all()
            .iter()
            .copied()
            .map(|embedder| (embedder, [0x42; 32]))
            .collect(),
        frozen_at,
        "0123456789abcdef0123456789abcdef01234567".to_string(),
    )?;
    Ok(TctConstellation::try_new(
        BTreeMap::from([(MutationCategory::KnownGood, category)]),
        BTreeMap::from([((Language::Python, MutationCategory::KnownGood), language)]),
        BTreeMap::from([(OracleOutcome::Pass, outcome)]),
        chunk,
        thresholds,
        provenance,
        frozen_at,
    )?)
}

fn normalized_fixture_vector(dim: usize, seed: f32) -> Vec<f32> {
    let mut values = (0..dim)
        .map(|idx| seed + idx as f32 * 0.01)
        .collect::<Vec<_>>();
    let norm = values.iter().map(|value| value * value).sum::<f32>().sqrt();
    for value in &mut values {
        *value /= norm;
    }
    values
}

fn write_train_cert_rows(db: &DB, count: usize) -> Result<()> {
    let cf = db
        .cf_handle(context_graph_mejepa_cf::CF_MEJEPA_TRAIN_CERTS)
        .context("train cert CF missing")?;
    for idx in 0..count {
        db.put_cf(cf, format!("cert-{idx:04}").as_bytes(), b"count-only")?;
    }
    Ok(())
}

fn open_runtime_rocksdb(path: &Path) -> Result<Arc<DB>> {
    let mut opts = Options::default();
    opts.create_if_missing(true);
    opts.create_missing_column_families(true);
    opts.set_paranoid_checks(true);
    let descriptors = context_graph_mejepa_cf::all_hygiene_referenced_cfs()
        .into_iter()
        .map(|cf| ColumnFamilyDescriptor::new(cf, Options::default()))
        .collect::<Vec<_>>();
    Ok(Arc::new(DB::open_cf_descriptors(&opts, path, descriptors)?))
}

fn scheduler_config(root: &Path) -> SelfOptimConfig {
    SelfOptimConfig {
        status_path: root.join("self_optimization_status.json"),
        hygiene_archive_root: root.join("archive"),
        witness_chain_path: root.join("witness-chain.bin"),
        test_outcome_root: root.join("test-outcomes"),
        ..SelfOptimConfig::default().with_all_periods(Duration::from_millis(10))
    }
}

fn dummy_panel() -> Result<Panel> {
    let filled_mask = (1u16 << InstrumentSlot::all().len()) - 1;
    Ok(Panel::try_new(vec![0.0; PANEL_DIM], filled_mask)?)
}

fn test_prediction() -> RealityPrediction {
    RealityPrediction::try_new(RealityPrediction {
        prediction_id: [0x46; 16],
        witness_hash: WitnessHash([0x47; 32]),
        task_id: TaskId("task-rwd-212-fsv".to_string()),
        session_id: [0x48; 16],
        language: InferLanguage::Python,
        covered_chunks: Vec::new(),
        verdict: Verdict::Pass,
        confidence_interval: ConformalInterval {
            lower: 0.6,
            upper: 0.8,
            ..ConformalInterval::default()
        },
        predicted_oracle_pass: 0.95,
        predicted_test_pass: vec![0.9, 0.8],
        predicted_runtime_trace: [0.0; 32],
        ood_score: 0.1,
        outcome_set: ConformalSet::try_new(vec![InferOracleOutcome::Pass], 0.1, 0.2).unwrap(),
        calibrated_confidence: 0.7,
        degraded_status: false,
        granger_attestations: BTreeMap::from([("fsv:constellation".to_string(), 0.9)]),
        predicted_failure_modes: Vec::new(),
        predicted_failed_tests: Vec::new(),
        guard_violations: Vec::new(),
        closest_exemplars: Vec::new(),
        predicted_edge_cases: Vec::new(),
        predicted_latent_bugs: Vec::new(),
        predicted_tech_debt_added: Vec::new(),
        predicted_dead_code: Vec::new(),
        predicted_redundant_code: Vec::new(),
        predicted_perf_regressions: Vec::new(),
        predicted_security_concerns: Vec::new(),
        predicted_accuracy_degradations: Vec::new(),
        predicted_cost_regressions: Vec::new(),
        predicted_reasoning_class: ReasoningClass::Mute,
        agent_claim_graph: AgentClaimGraph::default(),
        claim_reconciliation: Vec::new(),
        reality_impact: None,
        provenance: PredictionProvenance {
            predictor_version: "task-rwd-212-fsv".to_string(),
            constellation_version: "task-rwd-212-constellation".to_string(),
            calibration_version: "task-rwd-212-calibration".to_string(),
            active_pointer: hex::encode([0x46; 16]),
        },
        source_panel_sha: [0x5b; 32],
        calibration_version: "task-rwd-212-calibration".to_string(),
        created_at_unix_ms: 1_779_000_000_000,
        matched_fingerprint: None,
        unknown_candidate_id: None,
    })
    .unwrap()
}

fn test_context() -> TaskContext {
    TaskContext {
        task_id: TaskId("task-rwd-212-fsv".to_string()),
        session_id: [0x49; 16],
        language: InferLanguage::Python,
        problem_statement: "prove stale constellation freshness fails closed".to_string(),
        tests: vec![TestId(
            "tests/test_constellation.py::test_freshness".to_string(),
        )],
        environment: TaskEnvironment {
            repo_root: PathBuf::from("/tmp/task-rwd-212-fsv"),
            python_version: Some("3.12".to_string()),
            os: "linux".to_string(),
        },
        claim_graph: Some(ClaimGraph {
            chunks: Vec::new(),
            entities: Vec::new(),
        }),
        skill_citations: Vec::<SkillCitation>::new(),
    }
}

fn write_json_0600(path: &Path, value: &serde_json::Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let bytes = serde_json::to_vec_pretty(value)?;
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(&bytes)?;
    file.write_all(b"\n")?;
    Ok(())
}
