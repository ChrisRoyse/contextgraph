use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{ensure, Context, Result};
use context_graph_mejepa::calibration::count_cf;
use context_graph_mejepa::cli::write_json_0600;
use context_graph_mejepa::gates::{sha256_bytes, valid_witness_segment};
use context_graph_mejepa::heal::{read_status_snapshot, SchedulerState, SelfOptimConfig, TickName};
use context_graph_mejepa::{
    AstDiff, CalibrationExample, CalibrationStore, ColdCellMetric, DeterministicConstellationGuard,
    DeterministicOracleHead, DeterministicPredictor, DiffHunk, FailedGate, IdentityFrozenTarget,
    Language, MeJepaCompiler, MeJepaInferConfig, MejepaStore, PatchBundle, PatchWitnessReader,
    RocksDbEvalStore, RocksDbInferStore, TaskContext, TaskEnvironment, TaskId, TestId,
    TrainCertSummary, Verdict, VerifyVerdict,
};
use rocksdb::{ColumnFamilyDescriptor, IteratorMode, Options, WriteOptions, DB};
use serde_json::{json, Value};

const FSV_ROOT: &str = "/zfs/archive/contextgraph/fsv/task-py-g-061-cold-start-verdict-fsv";
const REPORT_NAME: &str = "cold_start_verdict_fsv.json";

fn main() -> Result<()> {
    let root = PathBuf::from(FSV_ROOT);
    if root.exists() {
        fs::remove_dir_all(&root).with_context(|| format!("remove {}", root.display()))?;
    }
    fs::create_dir_all(&root).with_context(|| format!("create {}", root.display()))?;
    let db_path = root.join("rocksdb");
    let repo_root = root.join("repo");
    fs::create_dir_all(&repo_root).with_context(|| format!("create {}", repo_root.display()))?;

    let db = open_full_db(&db_path)?;
    let calibration = CalibrationStore::new(db.clone(), 30)?;
    seed_model_state(db.as_ref(), &calibration)?;
    let store = Arc::new(RocksDbInferStore::new(db.clone()));

    let happy = run_verify_case(
        db.as_ref(),
        store.clone(),
        &repo_root,
        "happy_n12_abstains",
        Some(12),
        50,
        ExpectedCase::Cold(COLD_INSUFFICIENT),
    )?;
    let warm = run_verify_case(
        db.as_ref(),
        store.clone(),
        &repo_root,
        "edge_n78_allows_normal_verdict",
        Some(78),
        50,
        ExpectedCase::Approve,
    )?;
    let lookup_failure = run_verify_case(
        db.as_ref(),
        store.clone(),
        &repo_root,
        "edge_lookup_failure_abstains",
        None,
        50,
        ExpectedCase::Cold(COLD_LOOKUP),
    )?;
    let threshold_change = run_verify_case(
        db.as_ref(),
        store.clone(),
        &repo_root,
        "edge_threshold_100_applies_next_call",
        Some(78),
        100,
        ExpectedCase::Cold(COLD_INSUFFICIENT),
    )?;
    let warm_mid_session_cold = run_verify_case(
        db.as_ref(),
        store.clone(),
        &repo_root,
        "edge_warm_mid_session_first_cold",
        Some(49),
        50,
        ExpectedCase::Cold(COLD_INSUFFICIENT),
    )?;
    let warm_mid_session_hot = run_verify_case(
        db.as_ref(),
        store.clone(),
        &repo_root,
        "edge_warm_mid_session_second_warm",
        Some(120),
        50,
        ExpectedCase::Approve,
    )?;
    let invalid_threshold = invalid_threshold_case(db.as_ref(), store.clone(), &repo_root)?;
    let scheduler = scheduler_consumes_cold_metrics_case(&root, db.clone(), store.clone())?;

    let report_path = root.join(REPORT_NAME);
    let report = json!({
        "schema_version": 1,
        "task_id": "TASK-PY-G-061",
        "source_of_truth": {
            "db_path": db_path,
            "rocksdb_column_families": [
                context_graph_mejepa_cf::CF_MEJEPA_LIVE_PREDICTIONS,
                context_graph_mejepa_cf::CF_MEJEPA_COLD_CELL_METRICS,
                context_graph_mejepa_cf::CF_MEJEPA_PARK_LIST,
                context_graph_mejepa_cf::CF_MEJEPA_ACTIVE_LEARNING_QUEUE
            ],
            "report_path": report_path,
            "physical_db_files": physical_db_files(&db_path)?
        },
        "happy_path": happy,
        "edge_cases": [
            warm,
            lookup_failure,
            threshold_change,
            warm_mid_session_cold,
            warm_mid_session_hot,
            invalid_threshold,
            scheduler
        ],
        "all_passed": true
    });
    write_json_0600(&report_path, &report)?;
    let readback: Value = serde_json::from_slice(&fs::read(&report_path)?)?;
    let canonical: Value = serde_json::from_slice(&serde_json::to_vec(&report)?)?;
    ensure!(readback == canonical, "FSV report readback mismatch");
    println!("{}", serde_json::to_string_pretty(&readback)?);
    Ok(())
}

const CELL_ID: &str = "mutation=KnownGood:language=Python:entity=Function";
const COLD_INSUFFICIENT: &str = "COLD_CELL_INSUFFICIENT_SUPPORT";
const COLD_LOOKUP: &str = "COLD_CELL_LOOKUP_FAILURE";

#[derive(Debug, Clone, Copy)]
enum ExpectedCase {
    Approve,
    Cold(&'static str),
}

fn run_verify_case(
    db: &DB,
    store: Arc<RocksDbInferStore>,
    repo_root: &Path,
    scenario: &str,
    n_supporting: Option<u32>,
    threshold: u32,
    expected: ExpectedCase,
) -> Result<Value> {
    let before = physical_state(db, store.as_ref(), [9; 16])?;
    println!("{scenario} before={}", serde_json::to_string(&before)?);
    let compiler = compiler_with_support(repo_root, store.clone(), n_supporting, threshold)?;
    let (patch, context) = match expected {
        ExpectedCase::Approve => clean_patch_context(repo_root, scenario)?,
        ExpectedCase::Cold(_) => q4_patch_context(repo_root, scenario)?,
    };
    let verdict = compiler.verify(&patch, &context)?;
    let after = physical_state(db, store.as_ref(), context.session_id)?;
    println!("{scenario} after={}", serde_json::to_string(&after)?);

    match expected {
        ExpectedCase::Approve => {
            let VerifyVerdict::Approve {
                reality_prediction, ..
            } = &verdict
            else {
                anyhow::bail!("{scenario} expected approve, got {verdict:?}");
            };
            ensure!(
                reality_prediction.verdict != Verdict::Abstain,
                "{scenario} expected non-abstain normal prediction"
            );
            ensure!(
                after["cold_metric_count"] == before["cold_metric_count"],
                "{scenario} must not add cold-cell metric rows"
            );
        }
        ExpectedCase::Cold(reason) => {
            let VerifyVerdict::EscalateToHuman {
                reality_prediction: Some(prediction),
                failed_gate:
                    FailedGate::ColdCell {
                        reason: observed_reason,
                        threshold: observed_threshold,
                        ..
                    },
                ..
            } = &verdict
            else {
                anyhow::bail!("{scenario} expected cold-cell escalation, got {verdict:?}");
            };
            ensure!(observed_reason == reason, "{scenario} reason mismatch");
            ensure!(
                *observed_threshold == threshold,
                "{scenario} threshold mismatch"
            );
            ensure!(
                prediction.verdict == Verdict::Abstain,
                "{scenario} must persist Abstain"
            );
            ensure!(
                prediction
                    .granger_attestations
                    .contains_key("cold_cell_q4_note:emitted_under_abstain"),
                "{scenario} missing Q4 abstain note"
            );
            ensure!(
                !prediction.predicted_security_concerns.is_empty()
                    && !prediction.predicted_edge_cases.is_empty()
                    && !prediction.predicted_tech_debt_added.is_empty()
                    && !prediction.predicted_perf_regressions.is_empty()
                    && !prediction.predicted_accuracy_degradations.is_empty(),
                "{scenario} must preserve Q4 concern surfaces under abstain"
            );
            ensure!(
                after["cold_metric_count"].as_u64().unwrap()
                    == before["cold_metric_count"].as_u64().unwrap() + 1,
                "{scenario} must add exactly one cold metric"
            );
        }
    }
    Ok(json!({
        "case": scenario,
        "trigger": "MeJepaCompiler::verify",
        "n_supporting": n_supporting,
        "threshold": threshold,
        "before": before,
        "after": after,
        "verdict": verdict,
        "pass": true
    }))
}

fn q4_patch_context(repo_root: &Path, scenario: &str) -> Result<(PatchBundle, TaskContext)> {
    fs::create_dir_all(repo_root).with_context(|| format!("create {}", repo_root.display()))?;
    let rel_path = PathBuf::from(format!("{scenario}.py"));
    let path = repo_root.join(&rel_path);
    let after = format!(
        "API_KEY = \"AKIA0000000000000000\"\n\
         # TODO ownerless follow-up for {scenario}\n\
         def answer(items=None):\n\
             if items is None or len(items) == 0:\n\
                 return \"empty\"\n\
             for item in items:\n\
                 for other in items:\n\
                     pass\n\
             accuracy = 1.0\n\
             return accuracy\n"
    );
    fs::write(&path, after.as_bytes()).with_context(|| format!("write {}", path.display()))?;
    let post_sha = sha256_bytes(after.as_bytes());
    let patch = PatchBundle::try_new(
        AstDiff {
            hunks: vec![DiffHunk {
                path: rel_path,
                pre_sha: post_sha,
                post_sha,
                before: String::new(),
                after,
            }],
        },
        valid_witness_segment(),
        format!("TASK-PY-G-061 cold-cell FSV {scenario}"),
        sha256_bytes(format!("patch:{scenario}").as_bytes()),
    )?;
    let context = TaskContext {
        task_id: TaskId(format!("task-{scenario}")),
        session_id: [9; 16],
        language: Language::Python,
        problem_statement: format!("scenario: {scenario}"),
        tests: vec![TestId("test_empty_input".to_string())],
        environment: TaskEnvironment {
            repo_root: repo_root.to_path_buf(),
            python_version: Some("3.11".to_string()),
            os: std::env::consts::OS.to_string(),
        },
        claim_graph: None,
        skill_citations: Vec::new(),
    };
    Ok((patch, context))
}

fn clean_patch_context(repo_root: &Path, scenario: &str) -> Result<(PatchBundle, TaskContext)> {
    fs::create_dir_all(repo_root).with_context(|| format!("create {}", repo_root.display()))?;
    let rel_path = PathBuf::from(format!("{scenario}.py"));
    let path = repo_root.join(&rel_path);
    let after = format!("def answer():\n    return \"{scenario}\"\n");
    fs::write(&path, after.as_bytes()).with_context(|| format!("write {}", path.display()))?;
    let post_sha = sha256_bytes(after.as_bytes());
    let patch = PatchBundle::try_new(
        AstDiff {
            hunks: vec![DiffHunk {
                path: rel_path,
                pre_sha: post_sha,
                post_sha,
                before: String::new(),
                after,
            }],
        },
        valid_witness_segment(),
        format!("TASK-PY-G-061 clean warm-cell FSV {scenario}"),
        sha256_bytes(format!("patch:{scenario}").as_bytes()),
    )?;
    let context = TaskContext {
        task_id: TaskId(format!("task-{scenario}")),
        session_id: [9; 16],
        language: Language::Python,
        problem_statement: format!("scenario: {scenario}"),
        tests: vec![TestId("test_fixture".to_string())],
        environment: TaskEnvironment {
            repo_root: repo_root.to_path_buf(),
            python_version: Some("3.11".to_string()),
            os: std::env::consts::OS.to_string(),
        },
        claim_graph: None,
        skill_citations: Vec::new(),
    };
    Ok((patch, context))
}

fn compiler_with_support(
    repo_root: &Path,
    store: Arc<RocksDbInferStore>,
    n_supporting: Option<u32>,
    threshold: u32,
) -> Result<MeJepaCompiler> {
    let calibration = CalibrationStore::new(store.db(), 30)?;
    let config = MeJepaInferConfig {
        min_cell_support_for_verdict: threshold,
        outcome_set_max: 1,
        pause_state_path: None,
        ..MeJepaInferConfig::default()
    };
    Ok(MeJepaCompiler::new(
        config,
        Arc::new(DeterministicPredictor),
        Arc::new(IdentityFrozenTarget),
        Arc::new(DeterministicOracleHead),
        Arc::new(DeterministicConstellationGuard {
            version_id: "cold-cell-fsv-v1".to_string(),
            age_days: 0,
            cell_id: CELL_ID.to_string(),
            n_supporting,
            approved: true,
        }),
        Arc::new(PatchWitnessReader),
        store,
        calibration,
        repo_root.to_path_buf(),
    )?)
}

fn invalid_threshold_case(
    db: &DB,
    store: Arc<RocksDbInferStore>,
    repo_root: &Path,
) -> Result<Value> {
    let before = physical_state(db, store.as_ref(), [9; 16])?;
    let calibration = CalibrationStore::new(store.db(), 30)?;
    let config = MeJepaInferConfig {
        min_cell_support_for_verdict: 0,
        pause_state_path: None,
        ..MeJepaInferConfig::default()
    };
    let err = match MeJepaCompiler::new(
        config,
        Arc::new(DeterministicPredictor),
        Arc::new(IdentityFrozenTarget),
        Arc::new(DeterministicOracleHead),
        Arc::new(DeterministicConstellationGuard::default()),
        Arc::new(PatchWitnessReader),
        store.clone(),
        calibration,
        repo_root.to_path_buf(),
    ) {
        Ok(_) => anyhow::bail!("zero threshold unexpectedly built a compiler"),
        Err(err) => err,
    };
    let after = physical_state(db, store.as_ref(), [9; 16])?;
    ensure!(err.code() == "MEJEPA_INFER_PREDICTOR");
    ensure!(
        err.to_string().contains("MEJEPA_PRED_CONFIG_INVALID"),
        "invalid threshold must surface nested predictor config code"
    );
    ensure!(before == after, "invalid threshold must not mutate RocksDB");
    Ok(json!({
        "case": "edge_invalid_zero_threshold_fails_closed",
        "trigger": "MeJepaCompiler::new",
        "before": before,
        "after": after,
        "observed_error_code": err.code(),
        "pass": true
    }))
}

fn scheduler_consumes_cold_metrics_case(
    root: &Path,
    db: Arc<DB>,
    store: Arc<RocksDbInferStore>,
) -> Result<Value> {
    let now = chrono::Utc::now().timestamp_millis();
    for idx in 0..101u8 {
        let metric = ColdCellMetric::try_new(
            CELL_ID,
            COLD_INSUFFICIENT,
            Some(12),
            50,
            [idx; 16],
            format!("scheduler-cold-cell-{idx}"),
            [4; 16],
            now,
        )?;
        store.record_cold_cell_metric(&metric)?;
    }
    let status_path = root.join("self_optimization_status.json");
    let config = SelfOptimConfig {
        status_path: status_path.clone(),
        hygiene_archive_root: root.join("archive"),
        witness_chain_path: root.join("witness-chain.bin"),
        test_outcome_root: root.join("test-outcomes"),
        ..SelfOptimConfig::default()
    };
    let before_queue = count_cf(
        db.as_ref(),
        context_graph_mejepa_cf::CF_MEJEPA_ACTIVE_LEARNING_QUEUE,
    )?;
    let mut scheduler = SchedulerState::open(db.clone(), config)?;
    scheduler.run_tick(TickName::ActiveLearning)?;
    let eval_store = RocksDbEvalStore::new(db.clone())?;
    let queue = eval_store
        .load_queue()?
        .context("active learning queue missing after cold metric scheduler tick")?;
    let cold_entries = queue
        .entries
        .values()
        .filter(|entry| {
            matches!(
                entry.kind,
                context_graph_mejepa::ActiveLearningKind::ColdCellTargetedCorpus { .. }
            )
        })
        .count();
    let after_queue = count_cf(
        db.as_ref(),
        context_graph_mejepa_cf::CF_MEJEPA_ACTIVE_LEARNING_QUEUE,
    )?;
    ensure!(
        cold_entries == 1,
        "scheduler must queue one cold-cell target"
    );
    ensure!(
        after_queue == before_queue + 1,
        "active-learning queue row delta mismatch"
    );
    let status = read_status_snapshot(&status_path)?;
    let tick = status
        .last_success
        .get("active_learning")
        .context("missing active_learning tick")?;
    ensure!(
        tick.details["cold_cell_targets"].as_array().map(Vec::len) == Some(1),
        "tick details must include one cold-cell target"
    );
    Ok(json!({
        "case": "edge_scheduler_flags_cells_over_100_abstains_per_week",
        "trigger": "SchedulerState::run_tick(TickName::ActiveLearning)",
        "before_active_learning_queue_count": before_queue,
        "after_active_learning_queue_count": after_queue,
        "cold_entries": cold_entries,
        "tick_record": tick,
        "pass": true
    }))
}

fn physical_state(db: &DB, store: &RocksDbInferStore, session_id: [u8; 16]) -> Result<Value> {
    let predictions = store.read_live_predictions(session_id, 100)?;
    Ok(json!({
        "live_prediction_count": count_cf(db, context_graph_mejepa_cf::CF_MEJEPA_LIVE_PREDICTIONS)?,
        "cold_metric_count": count_cf(db, context_graph_mejepa_cf::CF_MEJEPA_COLD_CELL_METRICS)?,
        "park_list_count": count_cf(db, context_graph_mejepa_cf::CF_MEJEPA_PARK_LIST)?,
        "latest_session_prediction_verdicts": predictions.iter().map(|row| format!("{:?}", row.verdict)).collect::<Vec<_>>(),
        "cold_metric_rows": cold_metric_rows(db)?
    }))
}

fn cold_metric_rows(db: &DB) -> Result<Vec<Value>> {
    let cf = db
        .cf_handle(context_graph_mejepa_cf::CF_MEJEPA_COLD_CELL_METRICS)
        .context("missing cold-cell metric CF")?;
    let mut rows = Vec::new();
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let (_key, value) = item?;
        let metric: ColdCellMetric = bincode::deserialize(&value)?;
        metric.validate()?;
        rows.push(json!({
            "cell_id": metric.cell_id,
            "reason": metric.reason,
            "n_supporting": metric.n_supporting,
            "threshold": metric.threshold,
            "prediction_id": hex::encode(metric.prediction_id),
            "task_id": metric.task_id
        }));
    }
    Ok(rows)
}

fn seed_model_state(db: &DB, calibration: &CalibrationStore) -> Result<()> {
    let examples = (0..100)
        .map(|_idx| CalibrationExample {
            language: context_graph_mejepa::Language::Python,
            predicted_test_pass: vec![0.95],
            actual_test_pass: vec![1.0],
        })
        .collect::<Vec<_>>();
    calibration.calibrate(
        &examples,
        &[0.0025; 100],
        0.10,
        30,
        0.30,
        [7; 32],
        BTreeMap::new(),
    )?;
    let cf = db
        .cf_handle(context_graph_mejepa_cf::CF_MEJEPA_TRAIN_CERTS)
        .context("missing train cert CF")?;
    let cert = TrainCertSummary {
        step: 1,
        delta_omega: 0.9,
        delta_xi: 0.9,
        witness_offset: 1,
    };
    let mut opts = WriteOptions::default();
    opts.set_sync(true);
    db.put_cf_opt(
        cf,
        b"cert:task-py-g-061:0001",
        bincode::serialize(&cert)?,
        &opts,
    )?;
    Ok(())
}

fn open_full_db(path: &Path) -> Result<Arc<DB>> {
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

fn physical_db_files(db_path: &Path) -> Result<Vec<String>> {
    let mut files = fs::read_dir(db_path)?
        .map(|entry| {
            let entry = entry?;
            Ok(entry.file_name().to_string_lossy().to_string())
        })
        .collect::<Result<Vec<_>, std::io::Error>>()?;
    files.sort();
    Ok(files)
}
