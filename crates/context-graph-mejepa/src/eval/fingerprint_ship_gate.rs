use rocksdb::IteratorMode;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use super::error::{EvalError, EvalErrorCode};
use super::store::{
    RocksDbEvalStore, CF_MEJEPA_FINGERPRINT_SHIP_GATE_WINDOWS, CF_MEJEPA_MODEL_PROMOTIONS,
};

pub const FINGERPRINT_SHIP_GATE_REQUIRED_CONSECUTIVE_WINDOWS: usize = 4;
pub const FINGERPRINT_SHIP_GATE_ACCURACY_THRESHOLD: f32 = 0.95;
pub const FINGERPRINT_SHIP_GATE_PRECISION_THRESHOLD: f32 = 0.95;
pub const FINGERPRINT_SHIP_GATE_UNKNOWN_OOD_RECALL_THRESHOLD: f32 = 0.90;
pub const FINGERPRINT_SHIP_GATE_STABILITY_BLOCKER: &str =
    "MEJEPA_FINGERPRINT_SHIP_GATE_STABILITY_PENDING";

const MODEL_PROMOTION_RESET_PREFIXES: [&[u8]; 3] = [
    b"phase_e/per-cell-promotion/",
    b"phase_e/model-promotion/",
    b"model-promotion/",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct FingerprintClassificationMetrics {
    pub fingerprint_id: String,
    pub sample_count: u64,
    pub true_positive_count: u64,
    pub true_negative_count: u64,
    pub false_positive_count: u64,
    pub false_negative_count: u64,
    pub accuracy: f32,
    pub precision: f32,
    pub accuracy_threshold: f32,
    pub precision_threshold: f32,
    pub passed_threshold: bool,
}

impl FingerprintClassificationMetrics {
    pub fn validate(&self) -> Result<(), EvalError> {
        validate_fingerprint_id(&self.fingerprint_id, "fingerprint_id")?;
        if self.sample_count == 0 {
            return invalid("fingerprint sample_count must be greater than zero");
        }
        let observed_total = self.true_positive_count as u128
            + self.true_negative_count as u128
            + self.false_positive_count as u128
            + self.false_negative_count as u128;
        if observed_total != self.sample_count as u128 {
            return invalid(format!(
                "fingerprint {} count partition mismatch: tp+tn+fp+fn={} sample_count={}",
                self.fingerprint_id, observed_total, self.sample_count
            ));
        }
        let predicted_positive_count = self
            .true_positive_count
            .saturating_add(self.false_positive_count);
        if predicted_positive_count == 0 {
            return invalid(format!(
                "fingerprint {} precision is undefined because predicted_positive_count=0",
                self.fingerprint_id
            ));
        }
        validate_probability("accuracy", self.accuracy)?;
        validate_probability("precision", self.precision)?;
        validate_probability("accuracy_threshold", self.accuracy_threshold)?;
        validate_probability("precision_threshold", self.precision_threshold)?;
        let expected_accuracy =
            (self.true_positive_count + self.true_negative_count) as f32 / self.sample_count as f32;
        let expected_precision = self.true_positive_count as f32 / predicted_positive_count as f32;
        if !close_f32(self.accuracy, expected_accuracy) {
            return invalid(format!(
                "fingerprint {} accuracy {} does not match counts {}",
                self.fingerprint_id, self.accuracy, expected_accuracy
            ));
        }
        if !close_f32(self.precision, expected_precision) {
            return invalid(format!(
                "fingerprint {} precision {} does not match counts {}",
                self.fingerprint_id, self.precision, expected_precision
            ));
        }
        let expected_passed =
            self.accuracy >= self.accuracy_threshold && self.precision >= self.precision_threshold;
        if self.passed_threshold != expected_passed {
            return invalid(format!(
                "fingerprint {} passed_threshold {} does not match accuracy/precision thresholds",
                self.fingerprint_id, self.passed_threshold
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct UnknownOodRecallMetrics {
    pub actual_unknown_count: u64,
    pub detected_unknown_count: u64,
    pub missed_unknown_count: u64,
    pub recall: f32,
    pub recall_threshold: f32,
    pub passed_threshold: bool,
}

impl UnknownOodRecallMetrics {
    pub fn validate(&self) -> Result<(), EvalError> {
        if self.actual_unknown_count == 0 {
            return invalid("actual_unknown_count must be greater than zero");
        }
        let observed_total =
            self.detected_unknown_count as u128 + self.missed_unknown_count as u128;
        if observed_total != self.actual_unknown_count as u128 {
            return invalid(format!(
                "Unknown/OOD recall count mismatch: detected+missed={} actual_unknown_count={}",
                observed_total, self.actual_unknown_count
            ));
        }
        validate_probability("unknown_ood_recall", self.recall)?;
        validate_probability("unknown_ood_recall_threshold", self.recall_threshold)?;
        let expected_recall = self.detected_unknown_count as f32 / self.actual_unknown_count as f32;
        if !close_f32(self.recall, expected_recall) {
            return invalid(format!(
                "Unknown/OOD recall {} does not match counts {}",
                self.recall, expected_recall
            ));
        }
        let expected_passed = self.recall >= self.recall_threshold;
        if self.passed_threshold != expected_passed {
            return invalid(format!(
                "Unknown/OOD passed_threshold {} does not match recall threshold",
                self.passed_threshold
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct FingerprintShipGateWindow {
    pub window_id: String,
    pub report_date: String,
    pub generated_at_unix_ms: i64,
    pub per_fingerprint: BTreeMap<String, FingerprintClassificationMetrics>,
    pub unknown_ood_recall: UnknownOodRecallMetrics,
    pub passed_window: bool,
    pub failures: Vec<String>,
}

impl FingerprintShipGateWindow {
    pub fn validate(&self) -> Result<(), EvalError> {
        if self.window_id.trim().is_empty() {
            return invalid("fingerprint ship-gate window_id must be non-empty");
        }
        if self.window_id.as_bytes().contains(&0) {
            return invalid("fingerprint ship-gate window_id must not contain NUL bytes");
        }
        if self.report_date.trim().is_empty() {
            return invalid("fingerprint ship-gate report_date must be non-empty");
        }
        if self.generated_at_unix_ms < 0 {
            return invalid(format!(
                "fingerprint ship-gate generated_at_unix_ms must be non-negative; got {}",
                self.generated_at_unix_ms
            ));
        }
        if self.per_fingerprint.is_empty() {
            return invalid("fingerprint ship-gate window must contain per_fingerprint metrics");
        }
        let mut all_fingerprints_passed = true;
        for (fingerprint_id, metrics) in &self.per_fingerprint {
            validate_fingerprint_id(fingerprint_id, "per_fingerprint key")?;
            if fingerprint_id != &metrics.fingerprint_id {
                return invalid(format!(
                    "per_fingerprint key {} does not match payload fingerprint_id {}",
                    fingerprint_id, metrics.fingerprint_id
                ));
            }
            metrics.validate()?;
            all_fingerprints_passed &= metrics.passed_threshold;
        }
        self.unknown_ood_recall.validate()?;
        for failure in &self.failures {
            if failure.trim().is_empty() {
                return invalid("fingerprint ship-gate failures must not contain empty entries");
            }
        }
        let expected_passed = all_fingerprints_passed
            && self.unknown_ood_recall.passed_threshold
            && self.failures.is_empty();
        if self.passed_window != expected_passed {
            return invalid(format!(
                "fingerprint ship-gate window {} passed_window {} does not match metric failures",
                self.window_id, self.passed_window
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct FingerprintShipGateStabilityStatus {
    pub consecutive_passing_windows: usize,
    pub required_consecutive_windows: usize,
    pub ready: bool,
    pub latest_window_id: Option<String>,
    pub latest_report_date: Option<String>,
    pub latest_window_passed: bool,
    pub latest_min_accuracy: Option<f32>,
    pub latest_min_precision: Option<f32>,
    pub latest_unknown_ood_recall: Option<f32>,
    pub accuracy_threshold: f32,
    pub precision_threshold: f32,
    pub unknown_ood_recall_threshold: f32,
    pub evaluated_window_count: usize,
    pub model_promotion_reset_count: usize,
    pub latest_reset_reason: Option<String>,
    pub latest_reset_unix_ms: Option<i64>,
    pub latest_failures: Vec<String>,
}

pub fn fingerprint_ship_gate_stability_status(
    store: &RocksDbEvalStore,
) -> Result<FingerprintShipGateStabilityStatus, EvalError> {
    fingerprint_ship_gate_stability_status_with_requirements(
        store,
        FINGERPRINT_SHIP_GATE_REQUIRED_CONSECUTIVE_WINDOWS,
    )
}

pub fn fingerprint_ship_gate_stability_status_with_requirements(
    store: &RocksDbEvalStore,
    required_consecutive_windows: usize,
) -> Result<FingerprintShipGateStabilityStatus, EvalError> {
    if required_consecutive_windows == 0 {
        return Err(EvalError::new(
            EvalErrorCode::InvalidConfig,
            "required_consecutive_windows must be greater than zero",
        ));
    }
    let windows = store.load_fingerprint_ship_gate_windows_chronological()?;
    let resets = load_model_promotion_reset_events(store)?;
    Ok(compute_fingerprint_ship_gate_stability(
        &windows,
        &resets,
        required_consecutive_windows,
    ))
}

pub(crate) fn fingerprint_ship_gate_window_key(
    window: &FingerprintShipGateWindow,
) -> Result<Vec<u8>, EvalError> {
    window.validate()?;
    Ok(format!("{:020}:{}", window.generated_at_unix_ms, window.window_id).into_bytes())
}

fn load_model_promotion_reset_events(
    store: &RocksDbEvalStore,
) -> Result<Vec<ModelPromotionResetEvent>, EvalError> {
    let db = store.db();
    let cf = crate::calibration::cf(&db, CF_MEJEPA_MODEL_PROMOTIONS).map_err(EvalError::from)?;
    let mut resets = Vec::new();
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let (key, _value) = item?;
        if let Some(timestamp_millis) = model_promotion_reset_timestamp_from_key(key.as_ref()) {
            resets.push(ModelPromotionResetEvent {
                timestamp_millis,
                key: String::from_utf8_lossy(key.as_ref()).to_string(),
            });
        }
    }
    resets.sort_by(|left, right| {
        left.timestamp_millis
            .cmp(&right.timestamp_millis)
            .then_with(|| left.key.cmp(&right.key))
    });
    Ok(resets)
}

fn compute_fingerprint_ship_gate_stability(
    windows: &[FingerprintShipGateWindow],
    resets: &[ModelPromotionResetEvent],
    required_consecutive_windows: usize,
) -> FingerprintShipGateStabilityStatus {
    let mut consecutive_passing_windows = 0usize;
    let mut reset_idx = 0usize;
    let mut latest_reset_reason = None;
    let mut latest_reset_unix_ms = None;
    let mut latest_window_id = None;
    let mut latest_report_date = None;
    let mut latest_window_passed = false;
    let mut latest_min_accuracy = None;
    let mut latest_min_precision = None;
    let mut latest_unknown_ood_recall = None;
    let mut latest_failures = Vec::new();

    for window in windows {
        while let Some(reset) = resets.get(reset_idx) {
            if reset.timestamp_millis > window.generated_at_unix_ms {
                break;
            }
            consecutive_passing_windows = 0;
            latest_reset_reason = Some(format!("model_promotion:{}", reset.key));
            latest_reset_unix_ms = Some(reset.timestamp_millis);
            reset_idx += 1;
        }

        latest_window_id = Some(window.window_id.clone());
        latest_report_date = Some(window.report_date.clone());
        latest_window_passed = window.passed_window;
        latest_min_accuracy = window
            .per_fingerprint
            .values()
            .map(|metrics| metrics.accuracy)
            .min_by(|left, right| left.total_cmp(right));
        latest_min_precision = window
            .per_fingerprint
            .values()
            .map(|metrics| metrics.precision)
            .min_by(|left, right| left.total_cmp(right));
        latest_unknown_ood_recall = Some(window.unknown_ood_recall.recall);
        latest_failures = window.failures.clone();

        if latest_window_passed {
            consecutive_passing_windows += 1;
        } else {
            consecutive_passing_windows = 0;
            latest_reset_reason = Some(format!("failing_fingerprint_window:{}", window.window_id));
            latest_reset_unix_ms = Some(window.generated_at_unix_ms);
        }
    }

    while let Some(reset) = resets.get(reset_idx) {
        consecutive_passing_windows = 0;
        latest_reset_reason = Some(format!("model_promotion:{}", reset.key));
        latest_reset_unix_ms = Some(reset.timestamp_millis);
        reset_idx += 1;
    }

    if windows.is_empty() {
        latest_failures.push(format!(
            "{FINGERPRINT_SHIP_GATE_STABILITY_BLOCKER}: no rows in {CF_MEJEPA_FINGERPRINT_SHIP_GATE_WINDOWS}"
        ));
    }

    FingerprintShipGateStabilityStatus {
        consecutive_passing_windows,
        required_consecutive_windows,
        ready: consecutive_passing_windows >= required_consecutive_windows,
        latest_window_id,
        latest_report_date,
        latest_window_passed,
        latest_min_accuracy,
        latest_min_precision,
        latest_unknown_ood_recall,
        accuracy_threshold: FINGERPRINT_SHIP_GATE_ACCURACY_THRESHOLD,
        precision_threshold: FINGERPRINT_SHIP_GATE_PRECISION_THRESHOLD,
        unknown_ood_recall_threshold: FINGERPRINT_SHIP_GATE_UNKNOWN_OOD_RECALL_THRESHOLD,
        evaluated_window_count: windows.len(),
        model_promotion_reset_count: resets.len(),
        latest_reset_reason,
        latest_reset_unix_ms,
        latest_failures,
    }
}

fn validate_fingerprint_id(value: &str, field: &str) -> Result<(), EvalError> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return invalid(format!("{field} must be a 64-character hex fingerprint id"));
    }
    Ok(())
}

fn validate_probability(name: &str, value: f32) -> Result<(), EvalError> {
    if value.is_finite() && (0.0..=1.0).contains(&value) {
        Ok(())
    } else {
        invalid(format!("{name} must be finite in [0,1]; got {value}"))
    }
}

fn close_f32(left: f32, right: f32) -> bool {
    (left - right).abs() <= 0.000_001
}

fn invalid<T>(message: impl Into<String>) -> Result<T, EvalError> {
    Err(EvalError::new(EvalErrorCode::InvalidInput, message))
}

fn model_promotion_reset_timestamp_from_key(key: &[u8]) -> Option<i64> {
    for prefix in MODEL_PROMOTION_RESET_PREFIXES {
        let Some(rest) = key.strip_prefix(prefix) else {
            continue;
        };
        return parse_leading_i64(rest);
    }
    None
}

fn parse_leading_i64(bytes: &[u8]) -> Option<i64> {
    let end = bytes
        .iter()
        .position(|byte| !byte.is_ascii_digit())
        .unwrap_or(bytes.len());
    if end == 0 {
        return None;
    }
    std::str::from_utf8(&bytes[..end]).ok()?.parse().ok()
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ModelPromotionResetEvent {
    timestamp_millis: i64,
    key: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    const FP_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const FP_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    #[test]
    fn four_passing_windows_are_ready() {
        let windows = vec![
            passing_window("w1", 1_000),
            passing_window("w2", 2_000),
            passing_window("w3", 3_000),
            passing_window("w4", 4_000),
        ];
        let status = compute_fingerprint_ship_gate_stability(&windows, &[], 4);
        assert_eq!(status.consecutive_passing_windows, 4);
        assert!(status.ready);
        assert_eq!(status.latest_min_accuracy, Some(1.0));
        assert_eq!(status.latest_unknown_ood_recall, Some(0.9));
    }

    #[test]
    fn failing_window_resets_stability() {
        let windows = vec![
            passing_window("w1", 1_000),
            passing_window("w2", 2_000),
            failing_accuracy_window("w3", 3_000),
        ];
        let status = compute_fingerprint_ship_gate_stability(&windows, &[], 4);
        assert_eq!(status.consecutive_passing_windows, 0);
        assert!(!status.ready);
        assert!(status
            .latest_failures
            .iter()
            .any(|failure| { failure.contains(FP_A) && failure.contains("accuracy") }));
    }

    #[test]
    fn model_promotion_resets_stability() {
        let windows = vec![
            passing_window("w1", 1_000),
            passing_window("w2", 2_000),
            passing_window("w3", 3_000),
            passing_window("w4", 4_000),
        ];
        let resets = vec![ModelPromotionResetEvent {
            timestamp_millis: 5_000,
            key: "phase_e/per-cell-promotion/00000000000000005000".to_string(),
        }];
        let status = compute_fingerprint_ship_gate_stability(&windows, &resets, 4);
        assert_eq!(status.consecutive_passing_windows, 0);
        assert!(!status.ready);
        assert_eq!(status.model_promotion_reset_count, 1);
    }

    #[test]
    fn malformed_metric_counts_fail_closed() {
        let mut metric = passing_metrics(FP_A);
        metric.false_negative_count = 1;
        let err = metric.validate().unwrap_err();
        assert_eq!(err.code, EvalErrorCode::InvalidInput);
        assert!(err.message.contains("count partition mismatch"));
    }

    fn passing_window(window_id: &str, timestamp_millis: i64) -> FingerprintShipGateWindow {
        FingerprintShipGateWindow {
            window_id: window_id.to_string(),
            report_date: "2026-05-17".to_string(),
            generated_at_unix_ms: timestamp_millis,
            per_fingerprint: BTreeMap::from([
                (FP_A.to_string(), passing_metrics(FP_A)),
                (FP_B.to_string(), passing_metrics(FP_B)),
            ]),
            unknown_ood_recall: passing_unknown_recall(),
            passed_window: true,
            failures: Vec::new(),
        }
    }

    fn failing_accuracy_window(
        window_id: &str,
        timestamp_millis: i64,
    ) -> FingerprintShipGateWindow {
        let metric = FingerprintClassificationMetrics {
            fingerprint_id: FP_A.to_string(),
            sample_count: 20,
            true_positive_count: 10,
            true_negative_count: 8,
            false_positive_count: 0,
            false_negative_count: 2,
            accuracy: 0.90,
            precision: 1.0,
            accuracy_threshold: FINGERPRINT_SHIP_GATE_ACCURACY_THRESHOLD,
            precision_threshold: FINGERPRINT_SHIP_GATE_PRECISION_THRESHOLD,
            passed_threshold: false,
        };
        FingerprintShipGateWindow {
            window_id: window_id.to_string(),
            report_date: "2026-05-17".to_string(),
            generated_at_unix_ms: timestamp_millis,
            per_fingerprint: BTreeMap::from([
                (FP_A.to_string(), metric),
                (FP_B.to_string(), passing_metrics(FP_B)),
            ]),
            unknown_ood_recall: passing_unknown_recall(),
            passed_window: false,
            failures: vec![format!("fingerprint {FP_A} accuracy 0.900000 < 0.950000")],
        }
    }

    fn passing_metrics(fingerprint_id: &str) -> FingerprintClassificationMetrics {
        FingerprintClassificationMetrics {
            fingerprint_id: fingerprint_id.to_string(),
            sample_count: 20,
            true_positive_count: 18,
            true_negative_count: 2,
            false_positive_count: 0,
            false_negative_count: 0,
            accuracy: 1.0,
            precision: 1.0,
            accuracy_threshold: FINGERPRINT_SHIP_GATE_ACCURACY_THRESHOLD,
            precision_threshold: FINGERPRINT_SHIP_GATE_PRECISION_THRESHOLD,
            passed_threshold: true,
        }
    }

    fn passing_unknown_recall() -> UnknownOodRecallMetrics {
        UnknownOodRecallMetrics {
            actual_unknown_count: 10,
            detected_unknown_count: 9,
            missed_unknown_count: 1,
            recall: 0.9,
            recall_threshold: FINGERPRINT_SHIP_GATE_UNKNOWN_OOD_RECALL_THRESHOLD,
            passed_threshold: true,
        }
    }
}
