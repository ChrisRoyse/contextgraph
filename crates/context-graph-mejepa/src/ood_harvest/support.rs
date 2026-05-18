use crate::calibration::cf;
use crate::eval::{EvalError, EvalErrorCode, RocksDbEvalStore};
use crate::types::{PanelId, PredictionId, RealityPrediction};
use rocksdb::{IteratorMode, WriteOptions, DB};
use serde::de::DeserializeOwned;
use serde::Serialize;
use sha2::{Digest, Sha256};

use super::types::{
    invalid, validate_probability, OodHarvestQuarantineRow, OodHarvestReport, OodHarvestRow,
    OodHarvestStatus, OOD_HARVEST_ACTIVE_LEARNING_WEIGHT, OOD_HARVEST_SCHEMA_VERSION,
};

pub(super) fn empty_harvest_report(tiered_down_count: usize) -> OodHarvestReport {
    OodHarvestReport {
        scanned_live_predictions: 0,
        above_threshold_predictions: 0,
        harvested_count: 0,
        queued_count: 0,
        quarantined_count: 0,
        tiered_down_count,
        skipped_existing_count: 0,
        harvested_prediction_ids: Vec::new(),
        quarantine_codes: Vec::new(),
        source_live_prediction_cf: context_graph_mejepa_cf::CF_MEJEPA_LIVE_PREDICTIONS.to_string(),
        harvest_cf: context_graph_mejepa_cf::CF_MEJEPA_OOD_HARVEST.to_string(),
        calibration_cf: context_graph_mejepa_cf::CF_MEJEPA_OOD_CALIBRATIONS.to_string(),
        active_learning_queue_cf: context_graph_mejepa_cf::CF_MEJEPA_ACTIVE_LEARNING_QUEUE
            .to_string(),
    }
}

pub(super) fn row_from_prediction(
    eval_store: &RocksDbEvalStore,
    prediction: &RealityPrediction,
    harvested_at_unix_ms: i64,
) -> Result<OodHarvestRow, EvalError> {
    let oracle_outcome = eval_store
        .load_label(&prediction.task_id)?
        .map(|label| label.oracle_outcome);
    let row = OodHarvestRow {
        schema_version: OOD_HARVEST_SCHEMA_VERSION,
        prediction_id: PredictionId(prediction.prediction_id),
        task_id: prediction.task_id.clone(),
        session_id: prediction.session_id,
        panel_id: PanelId(prediction.source_panel_sha),
        affected_chunk_ids: prediction.covered_chunks.clone(),
        agent_prose: prediction.agent_claim_graph.raw_response.clone(),
        verdict: prediction.verdict,
        ood_score: prediction.ood_score,
        created_at_unix_ms: prediction.created_at_unix_ms,
        harvested_at_unix_ms,
        oracle_outcome,
        status: OodHarvestStatus::Active,
        priority_weight: OOD_HARVEST_ACTIVE_LEARNING_WEIGHT,
        source_live_prediction_cf: context_graph_mejepa_cf::CF_MEJEPA_LIVE_PREDICTIONS.to_string(),
    };
    row.validate()?;
    Ok(row)
}

pub(super) fn quarantine_from_prediction(
    prediction: &RealityPrediction,
    code: &str,
    detail: &str,
    observed_at_unix_ms: i64,
) -> OodHarvestQuarantineRow {
    OodHarvestQuarantineRow {
        schema_version: OOD_HARVEST_SCHEMA_VERSION,
        prediction_id: PredictionId(prediction.prediction_id),
        task_id: prediction.task_id.clone(),
        panel_id: PanelId(prediction.source_panel_sha),
        code: code.to_string(),
        detail: detail.to_string(),
        observed_at_unix_ms,
        source_live_prediction_cf: context_graph_mejepa_cf::CF_MEJEPA_LIVE_PREDICTIONS.to_string(),
    }
}

pub(super) fn validate_live_prediction_key(
    key: &[u8],
    prediction: &RealityPrediction,
) -> Result<(), EvalError> {
    if key.len() != 40 {
        return Err(invalid(format!(
            "live prediction key must be 40 bytes, got {}",
            key.len()
        )));
    }
    if key[0..16] != prediction.session_id {
        return Err(invalid("live prediction key session_id mismatch"));
    }
    let mut created_at = [0u8; 8];
    created_at.copy_from_slice(&key[16..24]);
    if i64::from_be_bytes(created_at) != prediction.created_at_unix_ms {
        return Err(invalid("live prediction key created_at_unix_ms mismatch"));
    }
    if key[24..40] != prediction.prediction_id {
        return Err(invalid("live prediction key prediction_id mismatch"));
    }
    Ok(())
}

pub(super) fn load_panel_bytes(db: &DB, panel_id: PanelId) -> Result<Option<Vec<u8>>, EvalError> {
    if panel_id.0.iter().all(|byte| *byte == 0) {
        return Ok(None);
    }
    let cf = cf(db, context_graph_mejepa_cf::CF_MEJEPA_PANELS)?;
    if let Some(bytes) = db.get_cf(cf, panel_id.0)? {
        return Ok(Some(bytes));
    }
    let panel_hex = hex::encode(panel_id.0);
    if let Some(bytes) = db.get_cf(cf, panel_hex.as_bytes())? {
        return Ok(Some(bytes));
    }
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let (key, value) = item?;
        if key.as_ref() == panel_id.0 || key.as_ref() == panel_hex.as_bytes() {
            return Ok(Some(value.to_vec()));
        }
        if contains_subslice(&value, panel_hex.as_bytes()) {
            return Ok(Some(value.to_vec()));
        }
    }
    Ok(None)
}

pub(super) fn panel_bytes_are_degenerate(bytes: &[u8]) -> Result<bool, EvalError> {
    if bytes.is_empty() {
        return Ok(true);
    }
    if let Ok(value) = serde_json::from_slice::<serde_json::Value>(bytes) {
        if let Some(norm) = value.get("norm") {
            return Ok(!json_number_is_positive_finite(norm));
        }
        if let Some(values) = value.get("panel_values").and_then(|value| value.as_array()) {
            if values.is_empty() {
                return Ok(true);
            }
            let mut norm_sq = 0.0f64;
            for value in values {
                let Some(number) = value.as_f64() else {
                    return Ok(true);
                };
                if !number.is_finite() {
                    return Ok(true);
                }
                norm_sq += number * number;
            }
            return Ok(norm_sq <= f64::EPSILON);
        }
    }
    if let Ok(text) = std::str::from_utf8(bytes) {
        let lower = text.to_ascii_lowercase();
        return Ok(lower.contains("nan_norm")
            || lower.contains("not-a-number")
            || lower.contains("\"nan\""));
    }
    Ok(false)
}

pub(super) fn put_readback_bin<T>(
    db: &DB,
    cf_name: &str,
    key: &[u8],
    value: &T,
) -> Result<(), EvalError>
where
    T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug,
{
    let cf = cf(db, cf_name)?;
    let bytes = bincode::serialize(value)?;
    let mut opts = WriteOptions::default();
    opts.set_sync(true);
    db.put_cf_opt(cf, key, &bytes, &opts)?;
    let readback = db.get_cf(cf, key)?.ok_or_else(|| {
        EvalError::new(
            EvalErrorCode::ReadbackMismatch,
            format!("missing readback for CF {cf_name}"),
        )
    })?;
    if readback != bytes {
        return Err(EvalError::new(
            EvalErrorCode::ReadbackMismatch,
            format!("bytes differ after write to CF {cf_name}"),
        ));
    }
    let decoded: T = bincode::deserialize(&readback)?;
    if decoded != *value {
        return Err(EvalError::new(
            EvalErrorCode::ReadbackMismatch,
            format!("decoded row differs after write to CF {cf_name}"),
        ));
    }
    Ok(())
}

pub(super) fn status_rank(status: OodHarvestStatus) -> u8 {
    match status {
        OodHarvestStatus::Active => 0,
        OodHarvestStatus::DownweightedInDistribution => 1,
        OodHarvestStatus::TieredDown => 2,
    }
}

#[derive(Default)]
pub(super) struct ConfusionMatrix {
    pub(super) true_positive: usize,
    pub(super) false_positive: usize,
    pub(super) true_negative: usize,
    pub(super) false_negative: usize,
}

impl ConfusionMatrix {
    pub(super) fn add(&mut self, predicted_ood: bool, actual_ood: bool) {
        match (predicted_ood, actual_ood) {
            (true, true) => self.true_positive += 1,
            (true, false) => self.false_positive += 1,
            (false, true) => self.false_negative += 1,
            (false, false) => self.true_negative += 1,
        }
    }

    pub(super) fn false_positive_rate(&self) -> Result<f32, EvalError> {
        let denom = self.false_positive + self.true_negative;
        if denom == 0 {
            return Ok(0.0);
        }
        let rate = self.false_positive as f32 / denom as f32;
        validate_probability("ood_calibration.false_positive_rate", rate)?;
        Ok(rate)
    }
}

pub(super) fn calibration_report_id(
    generated_at_unix_ms: i64,
    matrix: &ConfusionMatrix,
    harvested_rows: usize,
    synthetic_rows: usize,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(generated_at_unix_ms.to_be_bytes());
    hasher.update(matrix.true_positive.to_be_bytes());
    hasher.update(matrix.false_positive.to_be_bytes());
    hasher.update(matrix.true_negative.to_be_bytes());
    hasher.update(matrix.false_negative.to_be_bytes());
    hasher.update(harvested_rows.to_be_bytes());
    hasher.update(synthetic_rows.to_be_bytes());
    hex::encode(&hasher.finalize()[..16])
}

fn json_number_is_positive_finite(value: &serde_json::Value) -> bool {
    let number = match value {
        serde_json::Value::Number(number) => number.as_f64(),
        serde_json::Value::String(text) => text.parse::<f64>().ok(),
        _ => None,
    };
    matches!(number, Some(number) if number.is_finite() && number > 0.0)
}

fn contains_subslice(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && haystack
            .windows(needle.len())
            .any(|window| window == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn degenerate_panel_detection_rejects_zero_norm_json() {
        let degenerate = br#"{"panel_hash":"abc","panel_values":[0.0,0.0]}"#;
        let normal = br#"{"panel_hash":"abc","panel_values":[1.0,0.25]}"#;
        assert!(panel_bytes_are_degenerate(degenerate).unwrap());
        assert!(!panel_bytes_are_degenerate(normal).unwrap());
    }

    #[test]
    fn confusion_matrix_flags_over_flagging_rate() {
        let mut matrix = ConfusionMatrix::default();
        matrix.add(true, false);
        matrix.add(false, false);
        assert_eq!(matrix.false_positive_rate().unwrap(), 0.5);
        assert!(matrix.false_positive_rate().unwrap() > 0.20);
    }
}
