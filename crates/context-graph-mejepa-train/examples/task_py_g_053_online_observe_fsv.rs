use context_graph_mejepa::heal::{SchedulerState, SelfOptimConfig, TickName};
use context_graph_mejepa::store::RocksDbInferStore;
use context_graph_mejepa::system_cost::SystemCostCounters;
use context_graph_mejepa::{
    count_sampler_reward_signals, persist_sampler_reward_signal_readback,
    read_sampler_reward_signal, sampler_reward_from_parts, ChunkId, ConformalInterval,
    ConformalSet, Language, MejepaStore, OracleOutcome, PredictionId, PredictionProvenance,
    RealityPrediction, RealityPredictionBuilder, SamplerRewardStatus, TaskId, Verdict,
};
use context_graph_mejepa_cf::{
    CF_MEJEPA_LIVE_PREDICTIONS, CF_MEJEPA_ORACLE_VERDICTS, CF_MEJEPA_PREDICTION_VERIFICATIONS,
    CF_MEJEPA_SAMPLER_REWARDS,
};
use context_graph_mejepa_train::config::TrainingConfig;
use context_graph_mejepa_train::sampler::{
    BatchSampler, OperatorOverrideSamplerInput, PatchSimilarityGraph, SamplerExampleSource,
};
use rocksdb::{ColumnFamilyDescriptor, Options, DB};
use serde_json::json;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

const HAPPY_EVENT_ID: &str = "53535353535353535353535353535353";

fn main() -> Result<()> {
    let fsv_root = PathBuf::from("/mnt/d/contextgraph/fsv/task-py-g-053-online-observe-fsv");
    fs::create_dir_all(&fsv_root)?;
    let run_root = fsv_root.join(format!(
        "run-{}-{}",
        chrono::Utc::now().format("%Y%m%dT%H%M%SZ"),
        std::process::id()
    ));
    fs::create_dir_all(&run_root)?;

    let cases = vec![
        happy_path_case(&run_root)?,
        no_oracle_case(&run_root)?,
        extreme_surprise_case(&run_root)?,
        duplicate_prediction_case(&run_root)?,
        stale_row_case(&run_root)?,
        non_finite_quarantine_case(&run_root)?,
    ];
    let all_passed = cases
        .iter()
        .all(|case| case["pass"].as_bool() == Some(true));
    let report = json!({
        "task_id": "TASK-PY-G-053",
        "issue": 272,
        "fsv_root": fsv_root.display().to_string(),
        "run_root": run_root.display().to_string(),
        "source_of_truth": {
            "reward_cf": CF_MEJEPA_SAMPLER_REWARDS,
            "live_prediction_cf": CF_MEJEPA_LIVE_PREDICTIONS,
            "oracle_cf": CF_MEJEPA_ORACLE_VERDICTS,
            "verification_cf": CF_MEJEPA_PREDICTION_VERIFICATIONS
        },
        "cases": cases,
        "all_passed": all_passed
    });

    let artifact_path = run_root.join("online_observe_fsv.json");
    write_report_readback(&artifact_path, &report)?;
    let readback: serde_json::Value = serde_json::from_slice(&fs::read(&artifact_path)?)?;
    if readback != report {
        return Err("TASK_PY_G_053_FSV_REPORT_READBACK_MISMATCH".into());
    }
    if !all_passed {
        return Err("TASK_PY_G_053_ONLINE_OBSERVE_FSV_FAILED".into());
    }
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn happy_path_case(run_root: &Path) -> Result<serde_json::Value> {
    let case_root = run_root.join("happy-pass-to-fail");
    let db_path = case_root.join("infer-db");
    let db = open_fsv_db(&db_path)?;
    let store = RocksDbInferStore::new(db.clone());
    let prediction_id = PredictionId([0x53; 16]);
    let session_id = [0x11; 16];
    let chunk = ChunkId("src/app.py#fn#happy".to_string());
    let prediction = prediction(
        prediction_id,
        session_id,
        "task-py-g-053-happy",
        chunk.clone(),
        0.8,
        Verdict::Pass,
        1_779_100_000_000,
    )?;
    let input = sampler_input(prediction_id, chunk.clone(), "task-py-g-053-happy");
    let base_sampler = base_sampler_from_input(&input)?;
    let before = json!({
        "sampler_reward_rows": count_sampler_reward_signals(db.as_ref())?,
        "row_exists": read_sampler_reward_signal(db.as_ref(), prediction_id)?.is_some(),
        "base_weight": base_sampler.weights[0]
    });

    store.write_live_prediction(&prediction)?;
    let config = scheduler_config(&case_root);
    write_test_outcome(
        &config.test_outcome_root,
        session_id,
        HAPPY_EVENT_ID,
        "fail",
        1_779_100_000_100,
    )?;
    let status_path = config.status_path.clone();
    let mut state = SchedulerState::open(db.clone(), config)?;
    state.run_tick(TickName::Observe)?;
    let status_json: serde_json::Value = serde_json::from_slice(&fs::read(&status_path)?)?;
    let reward = read_sampler_reward_signal(db.as_ref(), prediction_id)?
        .ok_or("happy path sampler reward row missing")?;
    let counters = Arc::new(SystemCostCounters::new());
    let (sampler, sampler_report) = BatchSampler::new_with_reward_signals(
        input.clone(),
        db.as_ref(),
        0,
        Some(counters.clone()),
    )?;
    let mut sampler_for_batch = sampler.clone();
    let batch = sampler_for_batch.next_batch(None, 2)?;
    let counter_after_batch = counters
        .snapshot()
        .online_reward_signals_applied_count_total;
    let after = json!({
        "sampler_reward_rows": count_sampler_reward_signals(db.as_ref())?,
        "status_code": reward.status_code(),
        "surprise_z": reward.surprise_z,
        "sampling_weight_multiplier": reward.sampling_weight_multiplier,
        "sampler_applied_count": sampler_report.applied_count,
        "boosted_weight": sampler.weights[0],
        "weight_ratio": sampler.weights[0] / base_sampler.weights[0],
        "batch": batch,
        "counter_after_batch": counter_after_batch,
        "observe_status": status_json["last_success"]["observe"]["details"]["test_outcome_bindings"].clone()
    });
    drop(state);
    drop(store);
    drop(db);
    let reopened_count = reopened_sampler_reward_count(&db_path)?;
    let pass = reward.status == SamplerRewardStatus::Ready
        && approx_eq(reward.surprise_z, 3.2)
        && approx_eq(reward.sampling_weight_multiplier, 4.2)
        && approx_eq(sampler.weights[0], base_sampler.weights[0] * 4.2)
        && sampler_report.applied_count == 1
        && counter_after_batch == 1
        && reopened_count == 1;
    Ok(json!({
        "case": "happy_path_pass_prediction_contradicted_by_fail_oracle",
        "source_of_truth": CF_MEJEPA_SAMPLER_REWARDS,
        "before": before,
        "after": after,
        "reopened_sampler_reward_rows": reopened_count,
        "expected": {
            "surprise_z": 3.2,
            "sampling_weight_multiplier": 4.2,
            "online_reward_signals_applied_count_total": 1
        },
        "pass": pass
    }))
}

fn no_oracle_case(run_root: &Path) -> Result<serde_json::Value> {
    let case_root = run_root.join("awaiting-oracle");
    let db_path = case_root.join("infer-db");
    let db = open_fsv_db(&db_path)?;
    let store = RocksDbInferStore::new(db.clone());
    let prediction_id = PredictionId([0x54; 16]);
    let session_id = [0x12; 16];
    let prediction = prediction(
        prediction_id,
        session_id,
        "task-py-g-053-awaiting",
        ChunkId("src/app.py#fn#awaiting".to_string()),
        0.8,
        Verdict::Pass,
        1_779_100_001_000,
    )?;
    store.write_live_prediction(&prediction)?;
    let config = scheduler_config(&case_root);
    fs::create_dir_all(&config.test_outcome_root)?;
    let status_path = config.status_path.clone();
    let before = json!({
        "sampler_reward_rows": count_sampler_reward_signals(db.as_ref())?,
        "row_exists": read_sampler_reward_signal(db.as_ref(), prediction_id)?.is_some()
    });
    let mut state = SchedulerState::open(db.clone(), config)?;
    state.run_tick(TickName::Observe)?;
    let status_json: serde_json::Value = serde_json::from_slice(&fs::read(&status_path)?)?;
    let after = json!({
        "sampler_reward_rows": count_sampler_reward_signals(db.as_ref())?,
        "row_exists": read_sampler_reward_signal(db.as_ref(), prediction_id)?.is_some(),
        "recorded_status": "AWAITING_ORACLE",
        "observe_status": status_json["last_success"]["observe"]["details"]["test_outcome_bindings"].clone()
    });
    drop(state);
    drop(store);
    drop(db);
    let reopened_count = reopened_sampler_reward_count(&db_path)?;
    let pass = after["sampler_reward_rows"] == json!(0)
        && after["row_exists"] == json!(false)
        && reopened_count == 0;
    Ok(json!({
        "case": "oracle_outcome_never_arrives_no_reward_row",
        "source_of_truth": CF_MEJEPA_SAMPLER_REWARDS,
        "before": before,
        "after": after,
        "expected": {"sampler_reward_rows": 0, "recorded_status": "AWAITING_ORACLE"},
        "reopened_sampler_reward_rows": reopened_count,
        "pass": pass
    }))
}

fn extreme_surprise_case(run_root: &Path) -> Result<serde_json::Value> {
    let case_root = run_root.join("extreme-surprise");
    let db_path = case_root.join("infer-db");
    let db = open_fsv_db(&db_path)?;
    let prediction_id = PredictionId([0x55; 16]);
    let chunk = ChunkId("src/app.py#fn#extreme".to_string());
    let input = sampler_input(prediction_id, chunk, "task-py-g-053-extreme");
    let base = base_sampler_from_input(&input)?;
    let before = json!({"sampler_reward_rows": count_sampler_reward_signals(db.as_ref())?});
    let row = sampler_reward_from_parts(
        prediction_id,
        12.0,
        "task-py-g-053-extreme::python",
        1_779_100_002_000,
        1_779_100_002_100,
        "55555555555555555555555555555555",
    )?;
    let write = persist_sampler_reward_signal_readback(db.as_ref(), &row)?;
    let (sampler, report) =
        BatchSampler::new_with_reward_signals(input.clone(), db.as_ref(), 0, None)?;
    let after = json!({
        "sampler_reward_rows": count_sampler_reward_signals(db.as_ref())?,
        "write_disposition": format!("{:?}", write.disposition),
        "status_code": row.status_code(),
        "surprise_z": row.surprise_z,
        "sampling_weight_multiplier": row.sampling_weight_multiplier,
        "weight_ratio": sampler.weights[0] / base.weights[0],
        "report": report
    });
    let pass = row.status == SamplerRewardStatus::ClampedExtremeSurprise
        && approx_eq(row.sampling_weight_multiplier, 6.0)
        && approx_eq(sampler.weights[0], base.weights[0] * 6.0)
        && report.applied_count == 1;
    drop(db);
    let reopened_count = reopened_sampler_reward_count(&db_path)?;
    Ok(json!({
        "case": "finite_extreme_surprise_clamped",
        "source_of_truth": CF_MEJEPA_SAMPLER_REWARDS,
        "before": before,
        "after": after,
        "expected": {"status_code": "CLAMPED_EXTREME_SURPRISE", "sampling_weight_multiplier": 6.0},
        "reopened_sampler_reward_rows": reopened_count,
        "pass": pass
    }))
}

fn duplicate_prediction_case(run_root: &Path) -> Result<serde_json::Value> {
    let case_root = run_root.join("duplicate-prediction");
    let db_path = case_root.join("infer-db");
    let db = open_fsv_db(&db_path)?;
    let prediction_id = PredictionId([0x56; 16]);
    let chunk = ChunkId("src/app.py#fn#duplicate".to_string());
    let input = sampler_input(prediction_id, chunk, "task-py-g-053-duplicate");
    let row = sampler_reward_from_parts(
        prediction_id,
        3.2,
        "task-py-g-053-duplicate::python",
        1_779_100_003_000,
        1_779_100_003_100,
        "56565656565656565656565656565656",
    )?;
    let before = json!({"sampler_reward_rows": count_sampler_reward_signals(db.as_ref())?});
    let first = persist_sampler_reward_signal_readback(db.as_ref(), &row)?;
    let second = persist_sampler_reward_signal_readback(db.as_ref(), &row)?;
    let (_sampler, report) =
        BatchSampler::new_with_reward_signals(input.clone(), db.as_ref(), 0, None)?;
    let after = json!({
        "sampler_reward_rows": count_sampler_reward_signals(db.as_ref())?,
        "first_disposition": format!("{:?}", first.disposition),
        "second_disposition": format!("{:?}", second.disposition),
        "sampler_applied_count": report.applied_count,
        "report": report
    });
    let pass = after["sampler_reward_rows"] == json!(1)
        && after["second_disposition"] == json!("Idempotent")
        && after["sampler_applied_count"] == json!(1);
    drop(db);
    let reopened_count = reopened_sampler_reward_count(&db_path)?;
    Ok(json!({
        "case": "duplicate_prediction_id_idempotent",
        "source_of_truth": CF_MEJEPA_SAMPLER_REWARDS,
        "before": before,
        "after": after,
        "expected": {"sampler_reward_rows": 1, "second_disposition": "Idempotent", "sampler_applied_count": 1},
        "reopened_sampler_reward_rows": reopened_count,
        "pass": pass
    }))
}

fn stale_row_case(run_root: &Path) -> Result<serde_json::Value> {
    let case_root = run_root.join("stale-row");
    let db_path = case_root.join("infer-db");
    let db = open_fsv_db(&db_path)?;
    let prediction_id = PredictionId([0x57; 16]);
    let chunk = ChunkId("src/app.py#fn#stale".to_string());
    let input = sampler_input(prediction_id, chunk, "task-py-g-053-stale");
    let base = base_sampler_from_input(&input)?;
    let row = sampler_reward_from_parts(
        prediction_id,
        3.2,
        "task-py-g-053-stale::python",
        1_779_100_004_000,
        100,
        "57575757575757575757575757575757",
    )?;
    let before = json!({"sampler_reward_rows": count_sampler_reward_signals(db.as_ref())?});
    persist_sampler_reward_signal_readback(db.as_ref(), &row)?;
    let (sampler, report) =
        BatchSampler::new_with_reward_signals(input.clone(), db.as_ref(), 100, None)?;
    let after = json!({
        "sampler_reward_rows": count_sampler_reward_signals(db.as_ref())?,
        "stale_count": report.stale_count,
        "sampler_applied_count": report.applied_count,
        "weight_ratio": sampler.weights[0] / base.weights[0],
        "report": report
    });
    let pass = report.stale_count == 1
        && report.applied_count == 0
        && approx_eq(sampler.weights[0], base.weights[0]);
    drop(db);
    let reopened_count = reopened_sampler_reward_count(&db_path)?;
    Ok(json!({
        "case": "stale_row_ignored_until_oracle_ts_advances",
        "source_of_truth": CF_MEJEPA_SAMPLER_REWARDS,
        "before": before,
        "after": after,
        "expected": {"stale_count": 1, "sampler_applied_count": 0, "weight_ratio": 1.0},
        "reopened_sampler_reward_rows": reopened_count,
        "pass": pass
    }))
}

fn non_finite_quarantine_case(run_root: &Path) -> Result<serde_json::Value> {
    let case_root = run_root.join("non-finite-quarantine");
    let db_path = case_root.join("infer-db");
    let db = open_fsv_db(&db_path)?;
    let prediction_id = PredictionId([0x58; 16]);
    let chunk = ChunkId("src/app.py#fn#nan".to_string());
    let input = sampler_input(prediction_id, chunk, "task-py-g-053-nan");
    let base = base_sampler_from_input(&input)?;
    let before = json!({"sampler_reward_rows": count_sampler_reward_signals(db.as_ref())?});
    let row = sampler_reward_from_parts(
        prediction_id,
        f32::NAN,
        "task-py-g-053-nan::python",
        1_779_100_005_000,
        1_779_100_005_100,
        "58585858585858585858585858585858",
    )?;
    persist_sampler_reward_signal_readback(db.as_ref(), &row)?;
    let (sampler, report) =
        BatchSampler::new_with_reward_signals(input.clone(), db.as_ref(), 0, None)?;
    let after = json!({
        "sampler_reward_rows": count_sampler_reward_signals(db.as_ref())?,
        "status_code": row.status_code(),
        "error_code": row.error_code,
        "sanitized_surprise_z": row.surprise_z,
        "sampling_weight_multiplier": row.sampling_weight_multiplier,
        "quarantined_non_finite_count": report.quarantined_non_finite_count,
        "sampler_applied_count": report.applied_count,
        "weight_ratio": sampler.weights[0] / base.weights[0],
        "report": report
    });
    let pass = row.status == SamplerRewardStatus::QuarantinedNonFinite
        && report.quarantined_non_finite_count == 1
        && report.applied_count == 0
        && approx_eq(sampler.weights[0], base.weights[0]);
    drop(db);
    let reopened_count = reopened_sampler_reward_count(&db_path)?;
    Ok(json!({
        "case": "non_finite_surprise_quarantined_and_not_applied",
        "source_of_truth": CF_MEJEPA_SAMPLER_REWARDS,
        "before": before,
        "after": after,
        "expected": {"status_code": "MEJEPA_SAMPLER_REWARD_NON_FINITE_SURPRISE", "sampler_applied_count": 0},
        "reopened_sampler_reward_rows": reopened_count,
        "pass": pass
    }))
}

fn scheduler_config(case_root: &Path) -> SelfOptimConfig {
    SelfOptimConfig {
        status_path: case_root.join("scheduler-status.json"),
        hygiene_archive_root: case_root.join("archive"),
        witness_chain_path: case_root.join("witness-chain.bin"),
        test_outcome_root: case_root.join("test-outcomes"),
        ..SelfOptimConfig::default().with_all_periods(Duration::from_millis(20))
    }
}

fn prediction(
    prediction_id: PredictionId,
    session_id: [u8; 16],
    task_id: &str,
    chunk: ChunkId,
    predicted_oracle_pass: f32,
    verdict: Verdict,
    created_at_unix_ms: i64,
) -> Result<RealityPrediction> {
    Ok(RealityPredictionBuilder::from_parts(
        TaskId(task_id.to_string()),
        session_id,
        Language::Python,
        ConformalSet::try_new(vec![OracleOutcome::Pass], 0.1, 0.2)?,
    )
    .prediction_id(prediction_id.0)
    .covered_chunks(vec![chunk])
    .verdict(verdict)
    .confidence_interval(ConformalInterval::default())
    .predicted_oracle_pass(predicted_oracle_pass)
    .predicted_test_pass(vec![predicted_oracle_pass])
    .predicted_runtime_trace([0.0; 32])
    .ood_score(0.1)
    .calibrated_confidence(0.8)
    .provenance(PredictionProvenance::default())
    .calibration_version("task-py-g-053-online-observe-fsv")
    .created_at_unix_ms(created_at_unix_ms)
    .build()?)
}

fn sampler_input(
    prediction_id: PredictionId,
    chunk: ChunkId,
    task_id: &str,
) -> OperatorOverrideSamplerInput {
    OperatorOverrideSamplerInput {
        config: TrainingConfig {
            random_seed: 53,
            batch_size: 2,
            ..TrainingConfig::default()
        },
        l_steps: vec![0.2, 0.2],
        overrides: vec![false; 2],
        ages_days: vec![0.0; 2],
        task_ids: vec![task_id.to_string(), format!("{task_id}-background")],
        example_sources: vec![
            SamplerExampleSource {
                prediction_id,
                chunk_id: chunk,
            },
            SamplerExampleSource {
                prediction_id: PredictionId([0x99; 16]),
                chunk_id: ChunkId("src/app.py#fn#background".to_string()),
            },
        ],
        patch_similarity_graph: PatchSimilarityGraph {
            neighbors: vec![Vec::new(); 2],
        },
    }
}

fn base_sampler_from_input(input: &OperatorOverrideSamplerInput) -> Result<BatchSampler> {
    Ok(BatchSampler::new(
        input.config.clone(),
        input.l_steps.clone(),
        input.overrides.clone(),
        input.ages_days.clone(),
        input.task_ids.clone(),
        input.patch_similarity_graph.clone(),
    )?)
}

fn write_test_outcome(
    root: &Path,
    session_id: [u8; 16],
    event_id: &str,
    outcome: &str,
    ts: i64,
) -> Result<()> {
    fs::create_dir_all(root)?;
    let session_hex = hex::encode(session_id);
    let event = json!({
        "event_id": event_id,
        "ts": ts,
        "session_id": session_hex,
        "tool_use_id": "task-py-g-053-fsv",
        "command": "pytest tests/test_online_reward.py",
        "framework": "pytest",
        "test_id": "tests/test_online_reward.py::test_oracle",
        "outcome": outcome,
        "duration_ms": 12,
        "error_log": if outcome == "fail" { "AssertionError: fsv contradiction" } else { "" },
        "source": "task-py-g-053-online-observe-fsv",
        "sequence": 0,
        "line_no": 1
    });
    fs::write(
        root.join(format!("{session_hex}.jsonl")),
        format!("{}\n", serde_json::to_string(&event)?),
    )?;
    Ok(())
}

fn reopened_sampler_reward_count(db_path: &Path) -> Result<u64> {
    let reopened = open_fsv_db(db_path)?;
    Ok(count_sampler_reward_signals(reopened.as_ref())?)
}

fn open_fsv_db(path: &Path) -> Result<Arc<DB>> {
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

fn write_report_readback(path: &Path, value: &serde_json::Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    {
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&tmp)?;
        let bytes = serde_json::to_vec_pretty(value)?;
        file.write_all(&bytes)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
    }
    fs::rename(&tmp, path)?;
    let readback: serde_json::Value = serde_json::from_slice(&fs::read(path)?)?;
    if readback != *value {
        return Err("TASK_PY_G_053_REPORT_READBACK_MISMATCH".into());
    }
    Ok(())
}

fn approx_eq(left: f32, right: f32) -> bool {
    (left - right).abs() <= 1e-5
}
