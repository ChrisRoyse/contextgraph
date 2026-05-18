use crate::calibration::cf;
use crate::eval::{ActiveLearningQueueState, EvalError, RocksDbEvalStore};
use crate::types::{OracleOutcome, PanelId, PredictionId, RealityPrediction, TaskId};
use rocksdb::{IteratorMode, DB};
use std::path::Path;
use std::sync::Arc;

mod support;
mod types;

use support::{
    calibration_report_id, empty_harvest_report, load_panel_bytes, panel_bytes_are_degenerate,
    put_readback_bin, quarantine_from_prediction, row_from_prediction, status_rank,
    validate_live_prediction_key, ConfusionMatrix,
};
pub use types::*;
use types::{invalid, validate_prediction_id};

pub fn ood_harvest_key(prediction_id: PredictionId) -> [u8; 16] {
    prediction_id.0
}

pub fn ood_harvest_quarantine_key(prediction_id: PredictionId) -> Vec<u8> {
    let mut key = b"quarantine:".to_vec();
    key.extend_from_slice(&prediction_id.0);
    key
}

pub fn ood_calibration_report_key(report: &OodCalibrationReport) -> Vec<u8> {
    let mut key = Vec::with_capacity(8 + report.report_id.len());
    key.extend_from_slice(&report.generated_at_unix_ms.to_be_bytes());
    key.extend_from_slice(report.report_id.as_bytes());
    key
}

pub fn ood_harvest_active_learning_task_id(prediction_id: PredictionId) -> TaskId {
    TaskId(format!("ood-harvest-{}", hex::encode(prediction_id.0)))
}

pub fn harvest_ood_predictions(
    db: Arc<DB>,
    config: &OodHarvestConfig,
    harvested_at_unix_ms: i64,
) -> Result<OodHarvestReport, EvalError> {
    config.validate()?;
    if harvested_at_unix_ms <= 0 {
        return Err(invalid("harvested_at_unix_ms must be positive"));
    }

    let tiered_down_count =
        tier_down_expired_ood_harvest_rows(db.as_ref(), config, harvested_at_unix_ms)?;
    let eval_store = RocksDbEvalStore::new(db.clone())?;
    let mut queue = eval_store
        .load_queue()?
        .unwrap_or(ActiveLearningQueueState::new(config.queue_capacity)?);
    let live_cf = cf(&db, context_graph_mejepa_cf::CF_MEJEPA_LIVE_PREDICTIONS)?;

    let mut report = empty_harvest_report(tiered_down_count);
    for item in db.iterator_cf(live_cf, IteratorMode::Start) {
        let (key, value) = item?;
        report.scanned_live_predictions += 1;
        let prediction: RealityPrediction = bincode::deserialize(&value)?;
        prediction.validate().map_err(EvalError::from)?;
        validate_live_prediction_key(&key, &prediction)?;
        if prediction.ood_score <= config.threshold {
            continue;
        }
        report.above_threshold_predictions += 1;
        let prediction_id = PredictionId(prediction.prediction_id);
        if read_ood_harvest_row(db.as_ref(), prediction_id)?.is_some()
            || read_ood_harvest_quarantine(db.as_ref(), prediction_id)?.is_some()
        {
            report.skipped_existing_count += 1;
            continue;
        }
        let panel_id = PanelId(prediction.source_panel_sha);
        let Some(panel_bytes) = load_panel_bytes(db.as_ref(), panel_id)? else {
            persist_ood_harvest_quarantine(
                db.as_ref(),
                &quarantine_from_prediction(
                    &prediction,
                    OOD_HARVEST_ORPHAN,
                    "source panel row missing from CF_MEJEPA_PANELS",
                    harvested_at_unix_ms,
                ),
            )?;
            report.quarantined_count += 1;
            report.quarantine_codes.push(OOD_HARVEST_ORPHAN.to_string());
            continue;
        };
        if panel_bytes_are_degenerate(&panel_bytes)? {
            persist_ood_harvest_quarantine(
                db.as_ref(),
                &quarantine_from_prediction(
                    &prediction,
                    OOD_HARVEST_DEGENERATE_PANEL,
                    "source panel row has non-finite or zero-norm evidence",
                    harvested_at_unix_ms,
                ),
            )?;
            report.quarantined_count += 1;
            report
                .quarantine_codes
                .push(OOD_HARVEST_DEGENERATE_PANEL.to_string());
            continue;
        }
        let row = row_from_prediction(&eval_store, &prediction, harvested_at_unix_ms)?;
        persist_ood_harvest_row(db.as_ref(), &row)?;
        queue.enqueue_ood_harvest(&row)?;
        report.harvested_count += 1;
        report.queued_count += 1;
        report
            .harvested_prediction_ids
            .push(hex::encode(row.prediction_id.0));
    }
    if report.queued_count > 0 {
        eval_store.persist_queue(&queue)?;
    }
    Ok(report)
}

pub fn persist_ood_harvest_row(db: &DB, row: &OodHarvestRow) -> Result<(), EvalError> {
    row.validate()?;
    let key = ood_harvest_key(row.prediction_id);
    put_readback_bin(
        db,
        context_graph_mejepa_cf::CF_MEJEPA_OOD_HARVEST,
        &key,
        row,
    )
}

pub fn read_ood_harvest_row(
    db: &DB,
    prediction_id: PredictionId,
) -> Result<Option<OodHarvestRow>, EvalError> {
    validate_prediction_id(prediction_id, "ood_harvest.prediction_id")?;
    let cf = cf(db, context_graph_mejepa_cf::CF_MEJEPA_OOD_HARVEST)?;
    let Some(bytes) = db.get_cf(cf, ood_harvest_key(prediction_id))? else {
        return Ok(None);
    };
    let row: OodHarvestRow = bincode::deserialize(&bytes)?;
    row.validate()?;
    Ok(Some(row))
}

pub fn persist_ood_harvest_quarantine(
    db: &DB,
    row: &OodHarvestQuarantineRow,
) -> Result<(), EvalError> {
    row.validate()?;
    put_readback_bin(
        db,
        context_graph_mejepa_cf::CF_MEJEPA_OOD_HARVEST,
        &ood_harvest_quarantine_key(row.prediction_id),
        row,
    )
}

pub fn read_ood_harvest_quarantine(
    db: &DB,
    prediction_id: PredictionId,
) -> Result<Option<OodHarvestQuarantineRow>, EvalError> {
    validate_prediction_id(prediction_id, "ood_harvest_quarantine.prediction_id")?;
    let cf = cf(db, context_graph_mejepa_cf::CF_MEJEPA_OOD_HARVEST)?;
    let Some(bytes) = db.get_cf(cf, ood_harvest_quarantine_key(prediction_id))? else {
        return Ok(None);
    };
    let row: OodHarvestQuarantineRow = bincode::deserialize(&bytes)?;
    row.validate()?;
    Ok(Some(row))
}

pub fn list_ood_harvest_rows(db: &DB) -> Result<Vec<OodHarvestRow>, EvalError> {
    let cf = cf(db, context_graph_mejepa_cf::CF_MEJEPA_OOD_HARVEST)?;
    let mut rows = Vec::new();
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let (key, value) = item?;
        if key.len() != 16 {
            continue;
        }
        let row: OodHarvestRow = bincode::deserialize(&value)?;
        row.validate()?;
        rows.push(row);
    }
    Ok(rows)
}

pub fn list_ood_harvest_quarantines(db: &DB) -> Result<Vec<OodHarvestQuarantineRow>, EvalError> {
    let cf = cf(db, context_graph_mejepa_cf::CF_MEJEPA_OOD_HARVEST)?;
    let mut rows = Vec::new();
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let (key, value) = item?;
        if !key.starts_with(b"quarantine:") {
            continue;
        }
        let row: OodHarvestQuarantineRow = bincode::deserialize(&value)?;
        row.validate()?;
        rows.push(row);
    }
    Ok(rows)
}

pub fn review_ood_harvest(
    db: &DB,
    db_path: Option<&Path>,
    top_n: usize,
) -> Result<Vec<OodHarvestReviewRow>, EvalError> {
    if top_n == 0 {
        return Err(invalid(
            "OOD harvest review top_n must be greater than zero",
        ));
    }
    let cf = cf(db, context_graph_mejepa_cf::CF_MEJEPA_OOD_HARVEST)?;
    let mut rows = Vec::new();
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let (key, value) = item?;
        if key.len() != 16 {
            continue;
        }
        let row: OodHarvestRow = bincode::deserialize(&value)?;
        row.validate()?;
        rows.push((key.to_vec(), value.len(), row));
    }
    rows.sort_by(|left, right| {
        status_rank(left.2.status)
            .cmp(&status_rank(right.2.status))
            .then_with(|| right.2.priority_weight.total_cmp(&left.2.priority_weight))
            .then_with(|| right.2.ood_score.total_cmp(&left.2.ood_score))
            .then_with(|| {
                right
                    .2
                    .harvested_at_unix_ms
                    .cmp(&left.2.harvested_at_unix_ms)
            })
            .then_with(|| left.2.prediction_id.cmp(&right.2.prediction_id))
    });
    Ok(rows
        .into_iter()
        .take(top_n)
        .map(|(key, value_len, row)| OodHarvestReviewRow {
            prediction_id_hex: hex::encode(row.prediction_id.0),
            task_id: row.task_id.0,
            ood_score: row.ood_score,
            status: row.status,
            priority_weight: row.priority_weight,
            harvested_at_unix_ms: row.harvested_at_unix_ms,
            panel_id_hex: hex::encode(row.panel_id.0),
            affected_chunk_count: row.affected_chunk_ids.len(),
            bytes_anchor: OodHarvestBytesAnchor {
                db_path: db_path.map(|path| path.display().to_string()),
                cf: context_graph_mejepa_cf::CF_MEJEPA_OOD_HARVEST.to_string(),
                key_hex: hex::encode(key),
                value_len,
            },
        })
        .collect())
}

pub fn tier_down_expired_ood_harvest_rows(
    db: &DB,
    config: &OodHarvestConfig,
    now_unix_ms: i64,
) -> Result<usize, EvalError> {
    config.validate()?;
    if now_unix_ms <= 0 {
        return Err(invalid("tier-down now_unix_ms must be positive"));
    }
    let mut changed = 0usize;
    for mut row in list_ood_harvest_rows(db)? {
        if row.status != OodHarvestStatus::Active {
            continue;
        }
        if now_unix_ms - row.harvested_at_unix_ms <= config.retention_active_ms {
            continue;
        }
        row.status = OodHarvestStatus::TieredDown;
        row.priority_weight = OOD_HARVEST_DOWNWEIGHTED_WEIGHT;
        persist_ood_harvest_row(db, &row)?;
        changed += 1;
    }
    Ok(changed)
}

pub fn downweight_ood_harvest_in_distribution(
    db: Arc<DB>,
    prediction_id: PredictionId,
    labeled_at_unix_ms: i64,
    reason: &str,
) -> Result<OodHarvestRow, EvalError> {
    validate_prediction_id(prediction_id, "ood_harvest_downweight.prediction_id")?;
    if labeled_at_unix_ms <= 0 {
        return Err(invalid(
            "OOD harvest downweight labeled_at_unix_ms must be positive",
        ));
    }
    if reason.trim().is_empty() {
        return Err(invalid("OOD harvest downweight reason must be non-empty"));
    }
    let mut row = read_ood_harvest_row(db.as_ref(), prediction_id)?.ok_or_else(|| {
        invalid(format!(
            "OOD harvest row not found for prediction {}",
            hex::encode(prediction_id.0)
        ))
    })?;
    row.status = OodHarvestStatus::DownweightedInDistribution;
    row.priority_weight = OOD_HARVEST_DOWNWEIGHTED_WEIGHT;
    row.oracle_outcome = Some(OracleOutcome::Pass);
    persist_ood_harvest_row(db.as_ref(), &row)?;

    let eval_store = RocksDbEvalStore::new(db)?;
    if let Some(mut queue) = eval_store.load_queue()? {
        let task_id = ood_harvest_active_learning_task_id(prediction_id);
        if let Some(entry) = queue.entries.get_mut(&task_id) {
            entry.score = OOD_HARVEST_DOWNWEIGHTED_WEIGHT;
            entry.curiosity_score = OOD_HARVEST_DOWNWEIGHTED_WEIGHT;
            entry.reason = format!("ood_harvest_downweighted:{reason}");
            entry.validate()?;
            eval_store.persist_queue(&queue)?;
        }
    }
    Ok(row)
}

pub fn compute_ood_calibration_report(
    db: &DB,
    config: &OodHarvestConfig,
    generated_at_unix_ms: i64,
    window_start_unix_ms: i64,
    window_end_unix_ms: i64,
    synthetic_rows: &[SyntheticOodCalibrationRow],
) -> Result<OodCalibrationReport, EvalError> {
    config.validate()?;
    if generated_at_unix_ms <= 0 {
        return Err(invalid(
            "OOD calibration generated_at_unix_ms must be positive",
        ));
    }
    let harvested = list_ood_harvest_rows(db)?;
    let mut matrix = ConfusionMatrix::default();
    for row in &harvested {
        if let Some(outcome) = row.oracle_outcome {
            matrix.add(
                row.ood_score > config.threshold,
                outcome == OracleOutcome::OutOfDistribution,
            );
        }
    }
    for row in synthetic_rows {
        row.validate()?;
        matrix.add(row.predicted_ood, row.actual_ood);
    }
    let false_positive_rate = matrix.false_positive_rate()?;
    let flags = if false_positive_rate > 0.20 {
        vec![OOD_GATE_OVER_FLAGGING.to_string()]
    } else {
        Vec::new()
    };
    let report_id = calibration_report_id(
        generated_at_unix_ms,
        &matrix,
        harvested.len(),
        synthetic_rows.len(),
    );
    let report = OodCalibrationReport {
        schema_version: OOD_CALIBRATION_SCHEMA_VERSION,
        report_id,
        generated_at_unix_ms,
        window_start_unix_ms,
        window_end_unix_ms,
        threshold: config.threshold,
        harvested_rows: harvested.len(),
        synthetic_ood_rows: synthetic_rows.len(),
        true_positive: matrix.true_positive,
        false_positive: matrix.false_positive,
        true_negative: matrix.true_negative,
        false_negative: matrix.false_negative,
        false_positive_rate,
        flags,
        source_harvest_cf: context_graph_mejepa_cf::CF_MEJEPA_OOD_HARVEST.to_string(),
        source_synthetic_cf: context_graph_mejepa_cf::CF_MEJEPA_SYNTHETIC_STRESS_RESULTS
            .to_string(),
    };
    persist_ood_calibration_report(db, &report)?;
    Ok(report)
}

pub fn persist_ood_calibration_report(
    db: &DB,
    report: &OodCalibrationReport,
) -> Result<(), EvalError> {
    report.validate()?;
    put_readback_bin(
        db,
        context_graph_mejepa_cf::CF_MEJEPA_OOD_CALIBRATIONS,
        &ood_calibration_report_key(report),
        report,
    )
}
