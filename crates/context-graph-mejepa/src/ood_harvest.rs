use crate::calibration::cf;
use crate::eval::{ActiveLearningQueueState, EvalError, RocksDbEvalStore};
use crate::types::{
    decode_reality_prediction, OracleOutcome, PanelId, PredictionId, TaskId, Verdict,
};
use rocksdb::{IteratorMode, DB};
use std::path::Path;
use std::sync::Arc;

mod support;
mod types;

use support::{
    auc_pairwise, calibration_report_id, empty_harvest_report, load_panel_bytes,
    matrix_at_threshold, panel_bytes_are_degenerate, put_readback_bin, quarantine_from_prediction,
    row_from_prediction, select_threshold, status_rank, validate_live_prediction_key,
};
pub use types::*;
use types::{invalid, validate_prediction_id};

const TASK_PY_G_014_MIN_AUC: f32 = 0.85;
const TASK_PY_G_014_EMPTY_HARVEST_MIN_AUC: f32 = 0.75;
const TASK_PY_G_014_MAX_ID_FALSE_POSITIVE_RATE: f32 = 0.05;
const TASK_PY_G_014_MIN_OOD_RECALL: f32 = 0.85;

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
        let prediction = decode_reality_prediction(&value).map_err(EvalError::from)?;
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
    let mut observations = Vec::<OodCalibrationObservation>::new();
    for row in &harvested {
        if let Some(outcome) = row.oracle_outcome {
            observations.push(OodCalibrationObservation {
                cell_id: row.calibration_cell.clone(),
                score: row.ood_score,
                actual_ood: outcome == OracleOutcome::OutOfDistribution,
            });
        }
    }
    for row in synthetic_rows {
        row.validate()?;
        observations.push(OodCalibrationObservation {
            cell_id: row.calibration_cell.clone(),
            score: row.ood_score,
            actual_ood: row.actual_ood,
        });
    }
    if observations.is_empty() {
        return Err(invalid(
            "OOD calibration requires at least one harvested or synthetic scored row",
        ));
    }
    let score_labels = observations
        .iter()
        .map(|row| (row.score, row.actual_ood))
        .collect::<Vec<_>>();
    let selected_threshold = select_threshold(
        &score_labels,
        config.threshold,
        TASK_PY_G_014_MAX_ID_FALSE_POSITIVE_RATE,
        TASK_PY_G_014_MIN_OOD_RECALL,
    )?;
    let matrix = matrix_at_threshold(&score_labels, selected_threshold)?;
    let false_positive_rate = matrix.false_positive_rate()?;
    let ood_recall = matrix.ood_recall()?;
    let id_scores = observations
        .iter()
        .filter(|row| !row.actual_ood)
        .map(|row| row.score)
        .collect::<Vec<_>>();
    let ood_scores = observations
        .iter()
        .filter(|row| row.actual_ood)
        .map(|row| row.score)
        .collect::<Vec<_>>();
    let global_auc = auc_pairwise(&ood_scores, &id_scores)?;
    let min_required_auc = if harvested.is_empty() {
        TASK_PY_G_014_EMPTY_HARVEST_MIN_AUC
    } else {
        TASK_PY_G_014_MIN_AUC
    };
    let cell_reports = build_ood_cell_reports(&observations, selected_threshold, min_required_auc)?;
    let mut flags = Vec::new();
    if harvested.is_empty() {
        flags.push(OOD_HARVEST_EMPTY.to_string());
    }
    if false_positive_rate > TASK_PY_G_014_MAX_ID_FALSE_POSITIVE_RATE {
        flags.push(OOD_GATE_OVER_FLAGGING.to_string());
    }
    if ood_recall < TASK_PY_G_014_MIN_OOD_RECALL {
        flags.push(OOD_RECALL_BELOW_TARGET.to_string());
    }
    if global_auc.is_none_or(|auc| auc < min_required_auc) {
        flags.push(OOD_AUC_BELOW_TARGET.to_string());
    }
    if cell_reports.iter().any(|cell| {
        cell.flags
            .iter()
            .any(|flag| flag == OOD_CELL_AUC_REGRESSION)
    }) {
        flags.push(OOD_CELL_AUC_REGRESSION.to_string());
    }
    flags.sort();
    flags.dedup();
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
        threshold: selected_threshold,
        harvested_rows: harvested.len(),
        synthetic_ood_rows: synthetic_rows.len(),
        id_rows: id_scores.len(),
        ood_rows: ood_scores.len(),
        true_positive: matrix.true_positive,
        false_positive: matrix.false_positive,
        true_negative: matrix.true_negative,
        false_negative: matrix.false_negative,
        global_auc,
        ood_recall,
        false_positive_rate,
        min_required_auc,
        selected_for_serving: flags.is_empty()
            || (harvested.is_empty() && flags.iter().all(|flag| flag == OOD_HARVEST_EMPTY)),
        flags,
        cell_reports,
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

pub fn read_latest_ood_calibration_report(
    db: &DB,
) -> Result<Option<OodCalibrationReport>, EvalError> {
    let cf = cf(db, context_graph_mejepa_cf::CF_MEJEPA_OOD_CALIBRATIONS)?;
    let mut latest: Option<OodCalibrationReport> = None;
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let (_key, value) = item?;
        let report: OodCalibrationReport = bincode::deserialize(&value)?;
        report.validate()?;
        if latest
            .as_ref()
            .is_none_or(|current| report.generated_at_unix_ms > current.generated_at_unix_ms)
        {
            latest = Some(report);
        }
    }
    Ok(latest)
}

pub fn apply_ood_gate_decision(
    base_verdict: Verdict,
    ood_score: f32,
    calibration: Option<&OodCalibrationReport>,
    cold_cell: Option<(Option<u32>, u32)>,
) -> Result<OodGateDecision, EvalError> {
    types::validate_probability("ood_gate.ood_score", ood_score)?;
    if let Some((n_supporting, threshold)) = cold_cell {
        if n_supporting.is_none_or(|n| n < threshold) {
            let decision = OodGateDecision {
                verdict: Verdict::Abstain,
                reason: crate::compiler::COLD_CELL_INSUFFICIENT_SUPPORT.to_string(),
                ood_score,
                threshold: calibration.map(|report| report.threshold),
            };
            decision.validate()?;
            return Ok(decision);
        }
    }
    let Some(calibration) = calibration else {
        let decision = OodGateDecision {
            verdict: Verdict::GuardRejected,
            reason: OOD_CALIBRATOR_MISSING.to_string(),
            ood_score,
            threshold: None,
        };
        decision.validate()?;
        return Ok(decision);
    };
    calibration.validate()?;
    if ood_score > calibration.threshold {
        let decision = OodGateDecision {
            verdict: Verdict::OutOfDistribution,
            reason: OOD_SCORE_ABOVE_THRESHOLD.to_string(),
            ood_score,
            threshold: Some(calibration.threshold),
        };
        decision.validate()?;
        return Ok(decision);
    }
    let decision = OodGateDecision {
        verdict: base_verdict,
        reason: "OOD_SCORE_WITHIN_THRESHOLD".to_string(),
        ood_score,
        threshold: Some(calibration.threshold),
    };
    decision.validate()?;
    Ok(decision)
}

#[derive(Debug, Clone)]
struct OodCalibrationObservation {
    cell_id: String,
    score: f32,
    actual_ood: bool,
}

fn build_ood_cell_reports(
    observations: &[OodCalibrationObservation],
    threshold: f32,
    min_required_auc: f32,
) -> Result<Vec<OodCalibrationCellReport>, EvalError> {
    let mut cell_ids = observations
        .iter()
        .map(|row| row.cell_id.clone())
        .collect::<Vec<_>>();
    cell_ids.sort();
    cell_ids.dedup();
    let mut reports = Vec::new();
    for cell_id in cell_ids {
        let cell = observations
            .iter()
            .filter(|row| row.cell_id == cell_id)
            .map(|row| (row.score, row.actual_ood))
            .collect::<Vec<_>>();
        let matrix = matrix_at_threshold(&cell, threshold)?;
        let id_scores = cell
            .iter()
            .filter(|(_, actual_ood)| !*actual_ood)
            .map(|(score, _)| *score)
            .collect::<Vec<_>>();
        let ood_scores = cell
            .iter()
            .filter(|(_, actual_ood)| *actual_ood)
            .map(|(score, _)| *score)
            .collect::<Vec<_>>();
        let auc = auc_pairwise(&ood_scores, &id_scores)?;
        let mut flags = Vec::new();
        if auc.is_none() {
            flags.push(OOD_CELL_INSUFFICIENT_SUPPORT.to_string());
        } else if auc.unwrap_or_default() < min_required_auc {
            flags.push(OOD_CELL_AUC_REGRESSION.to_string());
        }
        let report = OodCalibrationCellReport {
            cell_id,
            threshold,
            id_rows: id_scores.len(),
            ood_rows: ood_scores.len(),
            auc,
            false_positive_rate: matrix.false_positive_rate()?,
            ood_recall: matrix.ood_recall()?,
            flags,
        };
        report.validate()?;
        reports.push(report);
    }
    Ok(reports)
}
