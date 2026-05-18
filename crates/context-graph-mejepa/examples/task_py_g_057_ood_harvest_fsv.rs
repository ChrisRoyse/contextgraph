use anyhow::{ensure, Context, Result};
use context_graph_mejepa::cli::write_json_0600;
use context_graph_mejepa::{
    compute_ood_calibration_report, count_cf, downweight_ood_harvest_in_distribution,
    harvest_ood_predictions, list_ood_harvest_quarantines, list_ood_harvest_rows,
    ood_harvest_active_learning_task_id, open_infer_rocksdb, persist_ood_harvest_row,
    read_ood_harvest_quarantine, read_ood_harvest_row, review_ood_harvest, ActiveLearningKind,
    AgentClaimGraph, ConformalSet, Language, MejepaStore, OodHarvestConfig, OodHarvestRow,
    OodHarvestStatus, OracleOutcome, PredictionId, RealityPrediction, RealityPredictionBuilder,
    RocksDbEvalStore, RocksDbInferStore, SyntheticOodCalibrationRow, TaskId, Verdict, WitnessHash,
    DEFAULT_OOD_HARVEST_THRESHOLD, OOD_GATE_OVER_FLAGGING, OOD_HARVEST_ACTIVE_LEARNING_WEIGHT,
    OOD_HARVEST_DEGENERATE_PANEL, OOD_HARVEST_ORPHAN,
};
use context_graph_mejepa_cf::{
    CF_MEJEPA_ACTIVE_LEARNING_QUEUE, CF_MEJEPA_LIVE_PREDICTIONS, CF_MEJEPA_OOD_CALIBRATIONS,
    CF_MEJEPA_OOD_HARVEST, CF_MEJEPA_PANELS,
};
use rocksdb::{WriteOptions, DB};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

const REPORT_NAME: &str = "ood_harvest_fsv.json";
const FSV_SUBDIR: &str = "task-py-g-057-ood-harvest-fsv";

fn main() -> Result<()> {
    let run_id = format!("run-{}", chrono::Utc::now().format("%Y%m%dT%H%M%SZ"));
    let run_dir = fsv_root()?.join(run_id);
    fs::create_dir_all(&run_dir).context("create FSV run dir")?;
    let db_path = run_dir.join("ood-harvest.rocksdb");
    let now = chrono::Utc::now().timestamp_millis();
    let config = OodHarvestConfig {
        threshold: DEFAULT_OOD_HARVEST_THRESHOLD,
        retention_active_ms: 1_000,
        queue_capacity: 128,
    };

    let report = {
        let db = open_infer_rocksdb(&db_path).context("open OOD harvest FSV db")?;
        let infer_store = RocksDbInferStore::new(db.clone());
        let eval_store = RocksDbEvalStore::new(db.clone()).context("open eval store")?;
        let before = source_counts(db.as_ref())?;

        let happy_ids = seed_happy_session(db.as_ref(), &infer_store, now)?;
        let happy_harvest = harvest_ood_predictions(db.clone(), &config, now + 10_000)
            .context("run happy OOD harvest")?;
        let happy_rows = list_ood_harvest_rows(db.as_ref())?;
        let happy_queue = eval_store
            .load_queue()?
            .context("active-learning queue after happy harvest")?;
        let happy_queue_weights = happy_ids
            .iter()
            .map(|prediction_id| {
                let task_id = ood_harvest_active_learning_task_id(*prediction_id);
                let entry = happy_queue
                    .entries
                    .get(&task_id)
                    .context("happy OOD harvest queue entry missing")?;
                ensure!(
                    matches!(entry.kind, ActiveLearningKind::OodHarvest { .. }),
                    "queue entry kind must be OodHarvest"
                );
                Ok((entry.score, entry.reason.clone()))
            })
            .collect::<Result<Vec<_>>>()?;
        let happy_pass = happy_harvest.harvested_count == 5
            && happy_rows.len() == 5
            && happy_queue_weights.iter().all(|(score, _)| {
                (*score - OOD_HARVEST_ACTIVE_LEARNING_WEIGHT).abs() < f32::EPSILON
            });

        let orphan_id = seed_orphan_prediction(&infer_store, now + 20_000)?;
        let orphan_report = harvest_ood_predictions(db.clone(), &config, now + 21_000)
            .context("run orphan edge harvest")?;
        let orphan_quarantine = read_ood_harvest_quarantine(db.as_ref(), orphan_id)?
            .context("orphan quarantine row missing")?;
        let orphan_pass = orphan_quarantine.code == OOD_HARVEST_ORPHAN
            && read_ood_harvest_row(db.as_ref(), orphan_id)?.is_none()
            && orphan_report.quarantined_count == 1;

        let degenerate_id = seed_degenerate_prediction(db.as_ref(), &infer_store, now + 30_000)?;
        let degenerate_report = harvest_ood_predictions(db.clone(), &config, now + 31_000)
            .context("run degenerate-panel edge harvest")?;
        let degenerate_quarantine = read_ood_harvest_quarantine(db.as_ref(), degenerate_id)?
            .context("degenerate quarantine row missing")?;
        let degenerate_pass = degenerate_quarantine.code == OOD_HARVEST_DEGENERATE_PANEL
            && read_ood_harvest_row(db.as_ref(), degenerate_id)?.is_none()
            && degenerate_report.quarantined_count == 1;

        let ttl_id = seed_old_harvest_row(db.as_ref(), &happy_rows[0], now)?;
        let tiered_down =
            harvest_ood_predictions(db.clone(), &config, now + 40_000).context("run TTL edge")?;
        let ttl_row = read_ood_harvest_row(db.as_ref(), ttl_id)?.context("TTL row missing")?;
        let ttl_pass = tiered_down.tiered_down_count == 1
            && ttl_row.status == OodHarvestStatus::TieredDown
            && (ttl_row.priority_weight - 1.0).abs() < f32::EPSILON;

        let downweighted = downweight_ood_harvest_in_distribution(
            db.clone(),
            happy_ids[0],
            now + 50_000,
            "operator-marked-in-distribution",
        )
        .context("run operator downweight edge")?;
        let downweight_queue = eval_store
            .load_queue()?
            .context("queue missing after downweight")?;
        let downweight_task = ood_harvest_active_learning_task_id(happy_ids[0]);
        let downweight_entry = downweight_queue
            .entries
            .get(&downweight_task)
            .context("downweighted queue entry missing")?;
        let downweight_pass = downweighted.status == OodHarvestStatus::DownweightedInDistribution
            && downweighted.oracle_outcome == Some(OracleOutcome::Pass)
            && (downweight_entry.score - 1.0).abs() < f32::EPSILON;

        let calibration = compute_ood_calibration_report(
            db.as_ref(),
            &config,
            now + 60_000,
            now,
            now + 60_000,
            &[
                SyntheticOodCalibrationRow {
                    row_id: "synthetic-true-positive".to_string(),
                    predicted_ood: true,
                    actual_ood: true,
                },
                SyntheticOodCalibrationRow {
                    row_id: "synthetic-true-negative".to_string(),
                    predicted_ood: false,
                    actual_ood: false,
                },
            ],
        )
        .context("persist OOD calibration report")?;
        let calibration_pass = calibration
            .flags
            .contains(&OOD_GATE_OVER_FLAGGING.to_string())
            && calibration.false_positive_rate > 0.20
            && count_cf(db.as_ref(), CF_MEJEPA_OOD_CALIBRATIONS)? == 1;

        let review_rows = review_ood_harvest(db.as_ref(), Some(&db_path), 5)
            .context("review OOD harvest rows")?;
        let review_pass = review_rows.len() == 5
            && review_rows.iter().all(|row| {
                row.bytes_anchor.cf == CF_MEJEPA_OOD_HARVEST
                    && row.bytes_anchor.value_len > 0
                    && !row.bytes_anchor.key_hex.is_empty()
            });

        let after = source_counts(db.as_ref())?;
        let quarantines = list_ood_harvest_quarantines(db.as_ref())?;
        let all_passed = happy_pass
            && orphan_pass
            && degenerate_pass
            && ttl_pass
            && downweight_pass
            && calibration_pass
            && review_pass;

        json!({
            "task": "TASK-PY-G-057",
            "issue": 276,
            "allPassed": all_passed,
            "sourceOfTruth": {
                "dbPath": db_path,
                "livePredictionCf": CF_MEJEPA_LIVE_PREDICTIONS,
                "panelCf": CF_MEJEPA_PANELS,
                "harvestCf": CF_MEJEPA_OOD_HARVEST,
                "calibrationCf": CF_MEJEPA_OOD_CALIBRATIONS,
                "activeLearningQueueCf": CF_MEJEPA_ACTIVE_LEARNING_QUEUE,
            },
            "before": before,
            "after": after,
            "cases": {
                "happy50EditSession": {
                    "passed": happy_pass,
                    "liveRowsSeeded": 50,
                    "deliberatelyOodRows": 5,
                    "harvestedCount": happy_harvest.harvested_count,
                    "queueWeights": happy_queue_weights,
                },
                "missingPanelOrphan": {
                    "passed": orphan_pass,
                    "predictionId": hex::encode(orphan_id.0),
                    "code": orphan_quarantine.code,
                },
                "degeneratePanel": {
                    "passed": degenerate_pass,
                    "predictionId": hex::encode(degenerate_id.0),
                    "code": degenerate_quarantine.code,
                },
                "ttlTierDown": {
                    "passed": ttl_pass,
                    "predictionId": hex::encode(ttl_id.0),
                    "tieredDownCount": tiered_down.tiered_down_count,
                    "status": ttl_row.status,
                    "priorityWeight": ttl_row.priority_weight,
                },
                "operatorInDistributionLabel": {
                    "passed": downweight_pass,
                    "predictionId": hex::encode(happy_ids[0].0),
                    "status": downweighted.status,
                    "oracleOutcome": downweighted.oracle_outcome,
                    "queueScore": downweight_entry.score,
                },
                "overFlaggingCalibration": {
                    "passed": calibration_pass,
                    "reportId": calibration.report_id,
                    "falsePositiveRate": calibration.false_positive_rate,
                    "flags": calibration.flags,
                },
                "cliReviewSurface": {
                    "passed": review_pass,
                    "rowCount": review_rows.len(),
                    "firstAnchor": review_rows.first().map(|row| &row.bytes_anchor),
                }
            },
            "readback": {
                "harvestRows": list_ood_harvest_rows(db.as_ref())?.len(),
                "quarantineRows": quarantines.len(),
                "quarantineCodes": quarantines.iter().map(|row| row.code.clone()).collect::<Vec<_>>(),
            }
        })
    };

    let report_path = run_dir.join(REPORT_NAME);
    write_json_0600(&report_path, &report).context("write FSV report")?;
    let report_sha256 = sha256_file(&report_path)?;
    let readback: Value = serde_json::from_slice(&fs::read(&report_path)?)?;
    ensure!(
        readback.get("allPassed").and_then(Value::as_bool) == Some(true),
        "FSV report allPassed must be true"
    );
    println!(
        "task-py-g-057 OOD harvest FSV allPassed=true path={} sha256={}",
        report_path.display(),
        report_sha256
    );
    Ok(())
}

fn seed_happy_session(
    db: &DB,
    infer_store: &RocksDbInferStore,
    now: i64,
) -> Result<Vec<PredictionId>> {
    let mut ood_ids = Vec::new();
    for idx in 0..50 {
        let is_ood = idx < 5;
        let panel_id = id32(&format!("happy-panel-{idx}"));
        write_panel(db, panel_id, false)?;
        let prediction = build_prediction(
            idx,
            panel_id,
            if is_ood { 0.93 } else { 0.20 },
            if is_ood {
                Verdict::OutOfDistribution
            } else {
                Verdict::Pass
            },
            now + idx as i64,
            if is_ood {
                "synthetic OOD edit: Lua file, binary blob, or 50k single-line payload"
            } else {
                "synthetic in-distribution Python edit"
            },
        )?;
        if is_ood {
            ood_ids.push(PredictionId(prediction.prediction_id));
        }
        infer_store.write_live_prediction(&prediction)?;
    }
    Ok(ood_ids)
}

fn seed_orphan_prediction(infer_store: &RocksDbInferStore, now: i64) -> Result<PredictionId> {
    let panel_id = id32("orphan-panel-not-written");
    let prediction = build_prediction(
        70,
        panel_id,
        0.96,
        Verdict::OutOfDistribution,
        now,
        "synthetic OOD edit with missing source panel",
    )?;
    let id = PredictionId(prediction.prediction_id);
    infer_store.write_live_prediction(&prediction)?;
    Ok(id)
}

fn seed_degenerate_prediction(
    db: &DB,
    infer_store: &RocksDbInferStore,
    now: i64,
) -> Result<PredictionId> {
    let panel_id = id32("degenerate-panel-zero-norm");
    write_panel(db, panel_id, true)?;
    let prediction = build_prediction(
        71,
        panel_id,
        0.97,
        Verdict::OutOfDistribution,
        now,
        "synthetic OOD edit with zero-norm panel",
    )?;
    let id = PredictionId(prediction.prediction_id);
    infer_store.write_live_prediction(&prediction)?;
    Ok(id)
}

fn seed_old_harvest_row(db: &DB, template: &OodHarvestRow, now: i64) -> Result<PredictionId> {
    let mut row = template.clone();
    row.prediction_id = PredictionId(id16("ttl-tier-down-old-harvest"));
    row.task_id = TaskId("ood-harvest-ttl-edge".to_string());
    row.harvested_at_unix_ms = now - 10_000;
    row.status = OodHarvestStatus::Active;
    row.priority_weight = OOD_HARVEST_ACTIVE_LEARNING_WEIGHT;
    persist_ood_harvest_row(db, &row)?;
    Ok(row.prediction_id)
}

fn build_prediction(
    idx: usize,
    panel_id: [u8; 32],
    ood_score: f32,
    verdict: Verdict,
    created_at_unix_ms: i64,
    agent_prose: &str,
) -> Result<RealityPrediction> {
    let outcome_set = ConformalSet::try_new(
        vec![
            OracleOutcome::Pass,
            OracleOutcome::Fail,
            OracleOutcome::OutOfDistribution,
        ],
        0.10,
        0.90,
    )?;
    Ok(RealityPredictionBuilder::from_parts(
        TaskId(format!("task-py-g-057-synthetic-{idx:03}")),
        id16("task-py-g-057-session"),
        Language::Python,
        outcome_set,
    )
    .prediction_id(id16(&format!("task-py-g-057-prediction-{idx:03}")))
    .witness_hash(WitnessHash(id32(&format!(
        "task-py-g-057-witness-{idx:03}"
    ))))
    .covered_chunks(vec![context_graph_mejepa::ChunkId(format!(
        "chunk-py-g-057-{idx:03}"
    ))])
    .verdict(verdict)
    .predicted_oracle_pass(if verdict == Verdict::Pass { 0.90 } else { 0.10 })
    .predicted_test_pass(vec![if verdict == Verdict::Pass { 0.92 } else { 0.08 }])
    .ood_score(ood_score)
    .calibrated_confidence(0.90)
    .agent_claim_graph(AgentClaimGraph {
        raw_response: agent_prose.to_string(),
        claims: Vec::new(),
    })
    .source_panel_sha(panel_id)
    .calibration_version("task-py-g-057-fsv")
    .created_at_unix_ms(created_at_unix_ms)
    .build()?)
}

fn write_panel(db: &DB, panel_id: [u8; 32], degenerate: bool) -> Result<()> {
    let cf = db
        .cf_handle(CF_MEJEPA_PANELS)
        .context("CF_MEJEPA_PANELS missing")?;
    let value = if degenerate {
        json!({
            "panel_hash": hex::encode(panel_id),
            "panel_values": [0.0, 0.0],
        })
    } else {
        json!({
            "panel_hash": hex::encode(panel_id),
            "panel_values": [1.0, 0.25, 0.5],
        })
    };
    let bytes = serde_json::to_vec(&value)?;
    let mut opts = WriteOptions::default();
    opts.set_sync(true);
    db.put_cf_opt(cf, hex::encode(panel_id).as_bytes(), &bytes, &opts)?;
    let readback = db
        .get_cf(cf, hex::encode(panel_id).as_bytes())?
        .context("panel readback missing")?;
    ensure!(readback == bytes, "panel readback bytes differ");
    Ok(())
}

fn source_counts(db: &DB) -> Result<Value> {
    Ok(json!({
        "livePredictionCfRows": count_cf(db, CF_MEJEPA_LIVE_PREDICTIONS)?,
        "oodHarvestCfRows": count_cf(db, CF_MEJEPA_OOD_HARVEST)?,
        "oodCalibrationCfRows": count_cf(db, CF_MEJEPA_OOD_CALIBRATIONS)?,
        "activeLearningQueueCfRows": count_cf(db, CF_MEJEPA_ACTIVE_LEARNING_QUEUE)?,
        "panelCfRows": count_cf(db, CF_MEJEPA_PANELS)?,
    }))
}

fn fsv_root() -> Result<PathBuf> {
    if let Ok(root) = std::env::var("CG_TASK_PY_G_057_FSV_ROOT") {
        return Ok(PathBuf::from(root));
    }
    Ok(PathBuf::from("/tmp/contextgraph").join(FSV_SUBDIR))
}

fn id16(seed: &str) -> [u8; 16] {
    let digest = Sha256::digest(seed.as_bytes());
    let mut out = [0u8; 16];
    out.copy_from_slice(&digest[..16]);
    out
}

fn id32(seed: &str) -> [u8; 32] {
    let digest = Sha256::digest(seed.as_bytes());
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest[..32]);
    out
}

fn sha256_file(path: &Path) -> Result<String> {
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    Ok(format!("sha256:{}", hex::encode(Sha256::digest(&bytes))))
}
