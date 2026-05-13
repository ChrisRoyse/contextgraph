use context_graph_mejepa::compiler::MejepaStore;
use context_graph_mejepa::eval::{ActiveLearningKind, ActiveLearningQueueEntry};
use context_graph_mejepa::store::RocksDbInferStore;
use context_graph_mejepa::system_cost::SystemCostCounters;
use context_graph_mejepa::{
    open_infer_rocksdb, persist_operator_override, ChunkId, ConformalInterval, ConformalSet,
    Language, OperatorOverride, OracleOutcome, OverrideVerdict, PredictionId, PredictionProvenance,
    RealityPrediction, RealityPredictionBuilder, TaskId, Verdict,
    OPERATOR_OVERRIDE_SAMPLING_WEIGHT,
};
use context_graph_mejepa_cf::{
    CF_MEJEPA_ACTIVE_LEARNING_EVICTIONS, CF_MEJEPA_LIVE_PREDICTIONS, CF_MEJEPA_OPERATOR_OVERRIDES,
};
use context_graph_mejepa_train::config::TrainingConfig;
use context_graph_mejepa_train::sampler::{
    BatchSampler, OperatorOverrideSamplerInput, PatchSimilarityGraph, SamplerExampleSource,
};
use rocksdb::{IteratorMode, WriteOptions, DB};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn main() -> Result<()> {
    let fsv_root = PathBuf::from("/mnt/d/contextgraph/fsv/phase-f-override-sampler-fsv");
    fs::create_dir_all(&fsv_root)?;
    let run_root = fsv_root.join(format!(
        "run-{}-{}",
        chrono::Utc::now().timestamp_millis(),
        std::process::id()
    ));
    fs::create_dir_all(&run_root)?;
    let db_path = run_root.join("infer-db");
    let db = open_infer_rocksdb(&db_path)?;
    let store = RocksDbInferStore::new(db.clone());

    let prediction_id = PredictionId([0x83; 16]);
    let dismissed_prediction_id = PredictionId([0x84; 16]);
    let task_id = TaskId("task-predict-014-active".to_string());
    let dismissed_task_id = TaskId("task-predict-014-dismissed".to_string());
    let covered_a = ChunkId("src/lib.rs#fn#a".to_string());
    let covered_b = ChunkId("src/lib.rs#fn#b".to_string());

    store.write_live_prediction(&prediction(
        prediction_id,
        task_id.clone(),
        vec![covered_a.clone(), covered_b.clone()],
        1_778_660_000_000,
    )?)?;
    store.write_live_prediction(&prediction(
        dismissed_prediction_id,
        dismissed_task_id.clone(),
        vec![covered_a.clone()],
        1_778_660_001_000,
    )?)?;
    flush_cfs(db.as_ref())?;

    let counts_before = cf_counts(db.as_ref())?;
    let no_override = build_sampler(db.as_ref(), None)?;

    let override_record = OperatorOverride::new(
        prediction_id,
        OverrideVerdict::Fail,
        "operator corrected bad prediction".to_string(),
        "operator-1".to_string(),
        1_778_660_002_000,
    )?;
    persist_operator_override(db.as_ref(), &override_record)?;
    flush_cfs(db.as_ref())?;
    let counters = Arc::new(SystemCostCounters::new());
    let one_override = build_sampler(db.as_ref(), Some(counters.clone()))?;
    let mut sampler_for_batch = one_override.0.clone();
    let batch = sampler_for_batch.next_batch(None, 2)?;
    let counter_after_batch = counters
        .snapshot()
        .operator_override_sampler_applied_count_total;

    persist_operator_override(db.as_ref(), &override_record)?;
    flush_cfs(db.as_ref())?;
    let duplicate_override = build_sampler(db.as_ref(), None)?;

    let dismissed_record = OperatorOverride::new(
        dismissed_prediction_id,
        OverrideVerdict::Fail,
        "operator dismissed stale correction".to_string(),
        "operator-1".to_string(),
        1_778_660_003_000,
    )?;
    persist_operator_override(db.as_ref(), &dismissed_record)?;
    write_dismissal(db.as_ref(), &dismissed_task_id)?;
    flush_cfs(db.as_ref())?;
    let dismissed_override = build_sampler(db.as_ref(), None)?;
    let counts_after = cf_counts(db.as_ref())?;

    drop(store);
    drop(db);
    let reopened = open_infer_rocksdb(&db_path)?;
    let reopened_counts = cf_counts(reopened.as_ref())?;

    let base_weight = no_override.0.weights[0];
    let boosted_weight = one_override.0.weights[0];
    let duplicate_weight = duplicate_override.0.weights[0];
    let dismissed_weight = dismissed_override.0.weights[3];
    let readback_equal = counts_after == reopened_counts;
    let physical_artifacts = physical_artifacts(&run_root)?;
    let report = json!({
        "fsv_root": fsv_root.display().to_string(),
        "task_id": "TASK-PREDICT-014",
        "build_release_sha": git_head_short(),
        "source_of_truth": {
            "db_path": db_path.display().to_string(),
            "override_cf": CF_MEJEPA_OPERATOR_OVERRIDES,
            "live_prediction_cf": CF_MEJEPA_LIVE_PREDICTIONS,
            "eviction_cf": CF_MEJEPA_ACTIVE_LEARNING_EVICTIONS
        },
        "happy_path": [
            {
                "case": "one_operator_override_boosts_covered_chunks",
                "sot": format!("{CF_MEJEPA_OPERATOR_OVERRIDES} + {CF_MEJEPA_LIVE_PREDICTIONS}"),
                "trigger": "BatchSampler::new_with_operator_override_db",
                "expected": {"applied_count": 2, "weight_multiplier": OPERATOR_OVERRIDE_SAMPLING_WEIGHT},
                "actual": {
                    "applied_count": one_override.1.applied_count,
                    "boosted_indices": one_override.1.boosted_indices,
                    "base_weight": base_weight,
                    "boosted_weight": boosted_weight
                },
                "pass": one_override.1.applied_count == 2
                    && approx_eq(boosted_weight, base_weight * OPERATOR_OVERRIDE_SAMPLING_WEIGHT)
            },
            {
                "case": "per_batch_system_cost_counter",
                "sot": "SystemCostCounters::snapshot",
                "trigger": "BatchSampler::next_batch",
                "expected": {"operator_override_sampler_applied_count_total": 2},
                "actual": {"batch": batch, "counter_after_batch": counter_after_batch},
                "pass": counter_after_batch == 2
            }
        ],
        "boundary_cases": [
            {
                "case": "zero_overrides_no_boost",
                "expected": {"applied_count": 0},
                "actual": {"applied_count": no_override.1.applied_count, "weights": no_override.0.weights},
                "pass": no_override.1.applied_count == 0
            },
            {
                "case": "dismissed_override_no_boost",
                "expected": {"dismissed_override_count": 1, "dismissed_weight": base_weight},
                "actual": {
                    "dismissed_override_count": dismissed_override.1.dismissed_override_count,
                    "dismissed_weight": dismissed_weight
                },
                "pass": dismissed_override.1.dismissed_override_count == 1
                    && approx_eq(dismissed_weight, base_weight)
            },
            {
                "case": "same_override_twice_stays_six_x",
                "expected": {"applied_count": 2, "weight": base_weight * OPERATOR_OVERRIDE_SAMPLING_WEIGHT},
                "actual": {"applied_count": duplicate_override.1.applied_count, "weight": duplicate_weight},
                "pass": duplicate_override.1.applied_count == 2
                    && approx_eq(duplicate_weight, base_weight * OPERATOR_OVERRIDE_SAMPLING_WEIGHT)
            },
            {
                "case": "uncovered_chunk_not_boosted",
                "expected": {"uncovered_weight": base_weight},
                "actual": {"uncovered_weight": one_override.0.weights[2]},
                "pass": approx_eq(one_override.0.weights[2], base_weight)
            }
        ],
        "cf_counts_before": counts_before,
        "cf_counts_after": counts_after.clone(),
        "reopened_counts": reopened_counts.clone(),
        "physical_artifacts": physical_artifacts,
        "readback_equal": readback_equal,
    });

    let cases_passed = report["happy_path"]
        .as_array()
        .into_iter()
        .flatten()
        .chain(report["boundary_cases"].as_array().into_iter().flatten())
        .all(|case| case["pass"].as_bool() == Some(true));
    let all_passed = cases_passed
        && report["readback_equal"].as_bool() == Some(true)
        && report["physical_artifacts"]
            .as_array()
            .is_some_and(|artifacts| !artifacts.is_empty());
    let mut report = report;
    report["all_passed"] = json!(all_passed);
    let artifact_path = fsv_root.join("override_sampler_fsv.json");
    write_report_readback(&artifact_path, &report)?;
    let readback: serde_json::Value = serde_json::from_slice(&fs::read(&artifact_path)?)?;
    if readback != report {
        return Err("MEJEPA_OPERATOR_OVERRIDE_SAMPLER_FSV_READBACK_MISMATCH".into());
    }
    if !all_passed {
        return Err("MEJEPA_OPERATOR_OVERRIDE_SAMPLER_FSV_FAILED".into());
    }
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn build_sampler(
    db: &DB,
    counters: Option<Arc<SystemCostCounters>>,
) -> Result<(
    BatchSampler,
    context_graph_mejepa_train::sampler::OperatorOverrideSamplerReport,
)> {
    let cfg = TrainingConfig {
        random_seed: 83,
        batch_size: 4,
        ..TrainingConfig::default()
    };
    let prediction_id = PredictionId([0x83; 16]);
    let dismissed_prediction_id = PredictionId([0x84; 16]);
    let input = OperatorOverrideSamplerInput {
        config: cfg,
        l_steps: vec![0.2, 0.2, 0.2, 0.2],
        overrides: vec![false; 4],
        ages_days: vec![0.0; 4],
        task_ids: vec![
            "task-predict-014-active".to_string(),
            "task-predict-014-active".to_string(),
            "task-predict-014-active".to_string(),
            "task-predict-014-dismissed".to_string(),
        ],
        example_sources: vec![
            SamplerExampleSource {
                prediction_id,
                chunk_id: ChunkId("src/lib.rs#fn#a".to_string()),
            },
            SamplerExampleSource {
                prediction_id,
                chunk_id: ChunkId("src/lib.rs#fn#b".to_string()),
            },
            SamplerExampleSource {
                prediction_id,
                chunk_id: ChunkId("src/lib.rs#fn#uncovered".to_string()),
            },
            SamplerExampleSource {
                prediction_id: dismissed_prediction_id,
                chunk_id: ChunkId("src/lib.rs#fn#a".to_string()),
            },
        ],
        patch_similarity_graph: PatchSimilarityGraph {
            neighbors: vec![Vec::new(); 4],
        },
    };
    Ok(BatchSampler::new_with_operator_override_db(
        input, db, counters,
    )?)
}

fn prediction(
    prediction_id: PredictionId,
    task_id: TaskId,
    covered_chunks: Vec<ChunkId>,
    created_at_unix_ms: i64,
) -> Result<RealityPrediction> {
    let mut prediction = RealityPredictionBuilder::from_parts(
        task_id,
        [0x83; 16],
        Language::Rust,
        ConformalSet::try_new(vec![OracleOutcome::Fail], 0.1, 0.2)?,
    )
    .prediction_id(prediction_id.0)
    .covered_chunks(covered_chunks)
    .verdict(Verdict::Fail)
    .confidence_interval(ConformalInterval::default())
    .predicted_oracle_pass(0.2)
    .predicted_test_pass(vec![0.2])
    .predicted_runtime_trace([0.0; 32])
    .ood_score(0.1)
    .calibrated_confidence(0.8)
    .provenance(PredictionProvenance::default())
    .calibration_version("phase-f-override-sampler-fsv")
    .build()?;
    prediction.created_at_unix_ms = created_at_unix_ms;
    prediction.validate()?;
    Ok(prediction)
}

fn write_dismissal(db: &DB, task_id: &TaskId) -> Result<()> {
    let cf = db
        .cf_handle(CF_MEJEPA_ACTIVE_LEARNING_EVICTIONS)
        .ok_or("MEJEPA_OPERATOR_OVERRIDE_SAMPLER_EVICTION_CF_MISSING")?;
    let entry = ActiveLearningQueueEntry {
        task_id: task_id.clone(),
        score: 1.0,
        outcome_set_len: 1,
        ood_score: 0.0,
        reason: "operator_dismissed: stale override".to_string(),
        kind: ActiveLearningKind::Uncertainty,
    };
    let bytes = bincode::serialize(&entry)?;
    let mut opts = WriteOptions::default();
    opts.set_sync(true);
    db.put_cf_opt(cf, task_id.0.as_bytes(), bytes, &opts)?;
    db.flush_cf(cf)?;
    Ok(())
}

fn cf_counts(db: &DB) -> Result<serde_json::Value> {
    Ok(json!({
        CF_MEJEPA_OPERATOR_OVERRIDES: count_cf(db, CF_MEJEPA_OPERATOR_OVERRIDES)?,
        CF_MEJEPA_LIVE_PREDICTIONS: count_cf(db, CF_MEJEPA_LIVE_PREDICTIONS)?,
        CF_MEJEPA_ACTIVE_LEARNING_EVICTIONS: count_cf(db, CF_MEJEPA_ACTIVE_LEARNING_EVICTIONS)?,
    }))
}

fn count_cf(db: &DB, cf_name: &str) -> Result<usize> {
    let cf = db
        .cf_handle(cf_name)
        .ok_or("MEJEPA_OPERATOR_OVERRIDE_SAMPLER_CF_MISSING")?;
    let mut count = 0_usize;
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        item?;
        count += 1;
    }
    Ok(count)
}

fn flush_cfs(db: &DB) -> Result<()> {
    for cf_name in [
        CF_MEJEPA_OPERATOR_OVERRIDES,
        CF_MEJEPA_LIVE_PREDICTIONS,
        CF_MEJEPA_ACTIVE_LEARNING_EVICTIONS,
    ] {
        let cf = db
            .cf_handle(cf_name)
            .ok_or("MEJEPA_OPERATOR_OVERRIDE_SAMPLER_CF_MISSING")?;
        db.flush_cf(cf)?;
    }
    Ok(())
}

fn physical_artifacts(root: &Path) -> Result<Vec<serde_json::Value>> {
    let mut out = Vec::new();
    collect_artifacts(root, &mut out)?;
    Ok(out)
}

fn collect_artifacts(path: &Path, out: &mut Vec<serde_json::Value>) -> Result<()> {
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_artifacts(&path, out)?;
            continue;
        }
        if path.extension().and_then(|ext| ext.to_str()) == Some("sst")
            || path.file_name().and_then(|name| name.to_str()) == Some("CURRENT")
        {
            let bytes = fs::read(&path)?;
            out.push(json!({
                "path": path,
                "bytes": bytes.len(),
                "sha256": hex::encode(Sha256::digest(&bytes)),
            }));
        }
    }
    Ok(())
}

fn approx_eq(a: f32, b: f32) -> bool {
    (a - b).abs() <= 0.0001
}

fn write_report_readback(path: &Path, report: &serde_json::Value) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(report)?;
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(path)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    Ok(())
}

fn git_head_short() -> String {
    std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}
