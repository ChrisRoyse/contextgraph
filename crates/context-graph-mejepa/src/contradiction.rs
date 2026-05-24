use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::error::MejepaInferError;
use crate::types::{validate_probability, Verdict};

pub const CONTRADICTION_SCHEMA_VERSION: u16 = 1;
pub const MULTI_HEAD_CONTRADICTION: &str = "MULTI_HEAD_CONTRADICTION";
pub const CONTRADICTION_THRESHOLD_MISSING: &str = "CONTRADICTION_THRESHOLD_MISSING";
pub const SINGLE_HEAD_ONLY: &str = "SINGLE_HEAD_ONLY";
/// #624: declared so the inline string `"CONTRADICTION_CALIBRATION_INVALID:..."`
/// at `verdict_assembly.rs::detect_multi_head_contradiction`'s error fallback
/// uses the canonical constant. Operators grepping logs for the constant name
/// must find rows tagged with this exact prefix.
pub const CONTRADICTION_CALIBRATION_INVALID: &str = "CONTRADICTION_CALIBRATION_INVALID";
pub const CONTRADICTION_FALSE_ABSTAIN_LIMIT: f32 = 0.05;
pub const CONTRADICTION_RECALL_FLOOR: f32 = 0.80;

const MAX_CELL_ID_BYTES: usize = 512;
const MAX_SOURCE_BYTES: usize = 512;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContradictionThresholds {
    pub schema_version: u16,
    pub cell_id: String,
    pub tau_oracle: f32,
    pub tau_failure_count: u32,
    pub abstain_rate: f32,
    pub false_abstain_rate: f32,
    pub contradiction_recall: f32,
    pub legitimate_pass_throughput: f32,
    pub calibration_rows: u32,
    pub adversarial_rows: u32,
    pub legitimate_pass_rows: u32,
    pub calibrated_at_unix_ms: i64,
    pub source: String,
}

impl ContradictionThresholds {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        if self.schema_version != CONTRADICTION_SCHEMA_VERSION {
            return invalid(
                "contradiction_thresholds.schema_version",
                format!(
                    "expected {CONTRADICTION_SCHEMA_VERSION}; got {}",
                    self.schema_version
                ),
            );
        }
        validate_cell_id("contradiction_thresholds.cell_id", &self.cell_id)?;
        validate_probability("contradiction_thresholds.tau_oracle", self.tau_oracle)?;
        if self.tau_failure_count == 0 {
            return invalid(
                "contradiction_thresholds.tau_failure_count",
                "must be at least one head",
            );
        }
        validate_probability("contradiction_thresholds.abstain_rate", self.abstain_rate)?;
        validate_probability(
            "contradiction_thresholds.false_abstain_rate",
            self.false_abstain_rate,
        )?;
        validate_probability(
            "contradiction_thresholds.contradiction_recall",
            self.contradiction_recall,
        )?;
        validate_probability(
            "contradiction_thresholds.legitimate_pass_throughput",
            self.legitimate_pass_throughput,
        )?;
        if self.calibration_rows == 0 {
            return invalid(
                "contradiction_thresholds.calibration_rows",
                "must be non-zero",
            );
        }
        if self.adversarial_rows == 0 {
            return invalid(
                "contradiction_thresholds.adversarial_rows",
                "must be non-zero",
            );
        }
        if self.legitimate_pass_rows == 0 {
            return invalid(
                "contradiction_thresholds.legitimate_pass_rows",
                "must be non-zero",
            );
        }
        if self.calibrated_at_unix_ms <= 0 {
            return invalid(
                "contradiction_thresholds.calibrated_at_unix_ms",
                "must be positive",
            );
        }
        validate_text(
            "contradiction_thresholds.source",
            &self.source,
            MAX_SOURCE_BYTES,
        )?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContradictionCalibrationRow {
    pub cell_id: String,
    pub oracle_pass_confidence: f32,
    pub high_severity_failure_count: u32,
    pub security_concern_count: u32,
    pub legitimate_pass: bool,
    pub adversarial_holdout: bool,
    pub source: String,
}

impl ContradictionCalibrationRow {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        validate_cell_id("contradiction_calibration_row.cell_id", &self.cell_id)?;
        validate_probability(
            "contradiction_calibration_row.oracle_pass_confidence",
            self.oracle_pass_confidence,
        )?;
        if !self.legitimate_pass && !self.adversarial_holdout {
            return invalid(
                "contradiction_calibration_row.labels",
                "row must be legitimate_pass or adversarial_holdout",
            );
        }
        validate_text(
            "contradiction_calibration_row.source",
            &self.source,
            MAX_SOURCE_BYTES,
        )?;
        Ok(())
    }

    fn failure_signal_count(&self) -> u32 {
        self.high_severity_failure_count
            .saturating_add(self.security_concern_count)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContradictionCalibrationConfig {
    pub min_legitimate_pass_throughput: f32,
    pub min_contradiction_recall: f32,
    pub calibrated_at_unix_ms: i64,
}

impl Default for ContradictionCalibrationConfig {
    fn default() -> Self {
        Self {
            min_legitimate_pass_throughput: 0.95,
            min_contradiction_recall: CONTRADICTION_RECALL_FLOOR,
            calibrated_at_unix_ms: 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContradictionDecisionKind {
    NoContradiction,
    MultiHeadContradiction,
    ThresholdMissing,
    SingleHeadOnly,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContradictionDecision {
    pub kind: ContradictionDecisionKind,
    pub reason: String,
    pub cell_id: Option<String>,
    pub oracle_pass_confidence: f32,
    pub high_severity_failure_count: u32,
    pub security_concern_count: u32,
    pub tau_oracle: Option<f32>,
    pub tau_failure_count: Option<u32>,
    pub verdict_override: Option<Verdict>,
}

impl ContradictionDecision {
    pub fn no_contradiction(oracle_pass_confidence: f32) -> Self {
        Self {
            kind: ContradictionDecisionKind::NoContradiction,
            reason: "NO_CONTRADICTION".to_string(),
            cell_id: None,
            oracle_pass_confidence,
            high_severity_failure_count: 0,
            security_concern_count: 0,
            tau_oracle: None,
            tau_failure_count: None,
            verdict_override: None,
        }
    }
}

pub fn contradiction_threshold_key(cell_id: &str) -> Result<Vec<u8>, MejepaInferError> {
    validate_cell_id("contradiction_thresholds.cell_id", cell_id)?;
    Ok(cell_id.as_bytes().to_vec())
}

pub fn detect_multi_head_contradiction(
    cell_id: Option<&str>,
    oracle_pass_confidence: f32,
    high_severity_failure_count: u32,
    security_concern_count: u32,
    thresholds: Option<&ContradictionThresholds>,
) -> Result<ContradictionDecision, MejepaInferError> {
    validate_probability(
        "contradiction.oracle_pass_confidence",
        oracle_pass_confidence,
    )?;
    let failure_signal_count = high_severity_failure_count.saturating_add(security_concern_count);
    if failure_signal_count == 0 {
        return Ok(ContradictionDecision {
            kind: ContradictionDecisionKind::SingleHeadOnly,
            reason: SINGLE_HEAD_ONLY.to_string(),
            cell_id: cell_id.map(ToString::to_string),
            oracle_pass_confidence,
            high_severity_failure_count,
            security_concern_count,
            tau_oracle: None,
            tau_failure_count: None,
            verdict_override: None,
        });
    }
    let Some(cell_id) = cell_id else {
        return Ok(ContradictionDecision {
            kind: ContradictionDecisionKind::ThresholdMissing,
            reason: CONTRADICTION_THRESHOLD_MISSING.to_string(),
            cell_id: None,
            oracle_pass_confidence,
            high_severity_failure_count,
            security_concern_count,
            tau_oracle: None,
            tau_failure_count: None,
            verdict_override: Some(Verdict::Abstain),
        });
    };
    let Some(thresholds) = thresholds else {
        return Ok(ContradictionDecision {
            kind: ContradictionDecisionKind::ThresholdMissing,
            reason: CONTRADICTION_THRESHOLD_MISSING.to_string(),
            cell_id: Some(cell_id.to_string()),
            oracle_pass_confidence,
            high_severity_failure_count,
            security_concern_count,
            tau_oracle: None,
            tau_failure_count: None,
            verdict_override: Some(Verdict::Abstain),
        });
    };
    thresholds.validate()?;
    if thresholds.cell_id != cell_id {
        return invalid(
            "contradiction_thresholds.cell_id",
            format!(
                "threshold cell {} does not match inference cell {cell_id}",
                thresholds.cell_id
            ),
        );
    }
    let triggered = oracle_pass_confidence >= thresholds.tau_oracle
        && failure_signal_count >= thresholds.tau_failure_count;
    Ok(ContradictionDecision {
        kind: if triggered {
            ContradictionDecisionKind::MultiHeadContradiction
        } else {
            ContradictionDecisionKind::NoContradiction
        },
        reason: if triggered {
            MULTI_HEAD_CONTRADICTION.to_string()
        } else {
            "NO_CONTRADICTION".to_string()
        },
        cell_id: Some(cell_id.to_string()),
        oracle_pass_confidence,
        high_severity_failure_count,
        security_concern_count,
        tau_oracle: Some(thresholds.tau_oracle),
        tau_failure_count: Some(thresholds.tau_failure_count),
        verdict_override: triggered.then_some(Verdict::Abstain),
    })
}

pub fn calibrate_contradiction_thresholds(
    rows: &[ContradictionCalibrationRow],
    config: ContradictionCalibrationConfig,
) -> Result<Vec<ContradictionThresholds>, MejepaInferError> {
    validate_probability(
        "contradiction_calibration.min_legitimate_pass_throughput",
        config.min_legitimate_pass_throughput,
    )?;
    validate_probability(
        "contradiction_calibration.min_contradiction_recall",
        config.min_contradiction_recall,
    )?;
    if config.calibrated_at_unix_ms <= 0 {
        return invalid(
            "contradiction_calibration.calibrated_at_unix_ms",
            "must be positive",
        );
    }
    if rows.is_empty() {
        return invalid("contradiction_calibration.rows", "must not be empty");
    }
    let mut by_cell = BTreeMap::<String, Vec<ContradictionCalibrationRow>>::new();
    for row in rows {
        row.validate()?;
        by_cell
            .entry(row.cell_id.clone())
            .or_default()
            .push(row.clone());
    }
    let mut out = Vec::with_capacity(by_cell.len());
    for (cell_id, cell_rows) in by_cell {
        let legitimate_pass_rows = cell_rows.iter().filter(|row| row.legitimate_pass).count();
        let adversarial_rows = cell_rows
            .iter()
            .filter(|row| row.adversarial_holdout)
            .count();
        if legitimate_pass_rows == 0 || adversarial_rows == 0 {
            return invalid(
                "contradiction_calibration.cell_rows",
                format!("{cell_id} requires both legitimate_pass and adversarial_holdout rows"),
            );
        }
        let mut oracle_candidates = cell_rows
            .iter()
            .map(|row| row.oracle_pass_confidence)
            .collect::<Vec<_>>();
        oracle_candidates.extend([0.50, 0.70, 0.80, 0.85, 0.90, 0.95]);
        oracle_candidates.sort_by(f32::total_cmp);
        oracle_candidates.dedup_by(|left, right| (*left - *right).abs() <= 0.000_001);
        let max_failure_count = cell_rows
            .iter()
            .map(ContradictionCalibrationRow::failure_signal_count)
            .max()
            .unwrap_or(1)
            .max(1);

        let mut best: Option<ContradictionThresholds> = None;
        for tau_oracle in oracle_candidates {
            for tau_failure_count in 1..=max_failure_count {
                let metrics = threshold_metrics(&cell_rows, tau_oracle, tau_failure_count);
                if metrics.legitimate_pass_throughput < config.min_legitimate_pass_throughput {
                    continue;
                }
                let candidate = ContradictionThresholds {
                    schema_version: CONTRADICTION_SCHEMA_VERSION,
                    cell_id: cell_id.clone(),
                    tau_oracle,
                    tau_failure_count,
                    abstain_rate: metrics.abstain_rate,
                    false_abstain_rate: metrics.false_abstain_rate,
                    contradiction_recall: metrics.contradiction_recall,
                    legitimate_pass_throughput: metrics.legitimate_pass_throughput,
                    calibration_rows: cell_rows.len() as u32,
                    adversarial_rows: adversarial_rows as u32,
                    legitimate_pass_rows: legitimate_pass_rows as u32,
                    calibrated_at_unix_ms: config.calibrated_at_unix_ms,
                    source: "TASK-PY-G-044".to_string(),
                };
                candidate.validate()?;
                if best
                    .as_ref()
                    .map(|current| better_threshold(&candidate, current))
                    .unwrap_or(true)
                {
                    best = Some(candidate);
                }
            }
        }
        let Some(best) = best else {
            return invalid(
                "contradiction_calibration.threshold_search",
                format!("{cell_id} had no threshold satisfying legitimate-pass throughput"),
            );
        };
        if best.contradiction_recall < config.min_contradiction_recall {
            return invalid(
                "contradiction_calibration.contradiction_recall",
                format!(
                    "{cell_id} recall {} below {}",
                    best.contradiction_recall, config.min_contradiction_recall
                ),
            );
        }
        out.push(best);
    }
    Ok(out)
}

#[derive(Debug, Clone, Copy)]
struct ThresholdMetrics {
    abstain_rate: f32,
    false_abstain_rate: f32,
    contradiction_recall: f32,
    legitimate_pass_throughput: f32,
}

fn threshold_metrics(
    rows: &[ContradictionCalibrationRow],
    tau_oracle: f32,
    tau_failure_count: u32,
) -> ThresholdMetrics {
    let mut abstain = 0usize;
    let mut false_abstain = 0usize;
    let mut legitimate = 0usize;
    let mut adversarial = 0usize;
    let mut adversarial_caught = 0usize;
    for row in rows {
        let triggered = row.oracle_pass_confidence >= tau_oracle
            && row.failure_signal_count() >= tau_failure_count;
        abstain += usize::from(triggered);
        if row.legitimate_pass {
            legitimate += 1;
            false_abstain += usize::from(triggered);
        }
        if row.adversarial_holdout {
            adversarial += 1;
            adversarial_caught += usize::from(triggered);
        }
    }
    let false_abstain_rate = false_abstain as f32 / legitimate.max(1) as f32;
    ThresholdMetrics {
        abstain_rate: abstain as f32 / rows.len().max(1) as f32,
        false_abstain_rate,
        contradiction_recall: adversarial_caught as f32 / adversarial.max(1) as f32,
        legitimate_pass_throughput: 1.0 - false_abstain_rate,
    }
}

fn better_threshold(
    candidate: &ContradictionThresholds,
    current: &ContradictionThresholds,
) -> bool {
    candidate
        .contradiction_recall
        .total_cmp(&current.contradiction_recall)
        .then_with(|| {
            current
                .false_abstain_rate
                .total_cmp(&candidate.false_abstain_rate)
        })
        .then_with(|| candidate.tau_oracle.total_cmp(&current.tau_oracle))
        .then_with(|| current.tau_failure_count.cmp(&candidate.tau_failure_count))
        .is_gt()
}

fn validate_cell_id(field: &str, value: &str) -> Result<(), MejepaInferError> {
    validate_text(field, value, MAX_CELL_ID_BYTES)?;
    if value.as_bytes().contains(&0) {
        return invalid(field, "must not contain NUL bytes");
    }
    Ok(())
}

fn validate_text(field: &str, value: &str, max_bytes: usize) -> Result<(), MejepaInferError> {
    if value.trim().is_empty() {
        return invalid(field, "must not be empty");
    }
    if value.len() > max_bytes {
        return Err(MejepaInferError::DimMismatch {
            expected: max_bytes,
            actual: value.len(),
            context: field.to_string(),
        });
    }
    if value.chars().any(char::is_control) {
        return invalid(field, "must not contain control characters");
    }
    Ok(())
}

fn invalid<T>(field: &str, detail: impl Into<String>) -> Result<T, MejepaInferError> {
    Err(MejepaInferError::InvalidInput {
        field: field.to_string(),
        detail: detail.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(
        cell_id: &str,
        oracle_pass_confidence: f32,
        failure_count: u32,
        legitimate_pass: bool,
        adversarial_holdout: bool,
    ) -> ContradictionCalibrationRow {
        ContradictionCalibrationRow {
            cell_id: cell_id.to_string(),
            oracle_pass_confidence,
            high_severity_failure_count: failure_count,
            security_concern_count: 0,
            legitimate_pass,
            adversarial_holdout,
            source: format!("{cell_id}:{oracle_pass_confidence}:{failure_count}"),
        }
    }

    #[test]
    fn missing_threshold_fails_closed_only_when_multiple_heads_exist() {
        let single = detect_multi_head_contradiction(Some("python:known_good"), 0.92, 0, 0, None)
            .expect("single-head decision");
        assert_eq!(single.kind, ContradictionDecisionKind::SingleHeadOnly);
        assert_eq!(single.verdict_override, None);

        let missing = detect_multi_head_contradiction(Some("python:subtle_flip"), 0.92, 1, 0, None)
            .expect("missing threshold decision");
        assert_eq!(missing.kind, ContradictionDecisionKind::ThresholdMissing);
        assert_eq!(missing.verdict_override, Some(Verdict::Abstain));
    }

    #[test]
    fn calibrated_threshold_catches_adversarial_rows_without_false_abstain() {
        let rows = vec![
            row("python:subtle_flip", 0.99, 0, true, false),
            row("python:subtle_flip", 0.90, 0, true, false),
            row("python:subtle_flip", 0.91, 2, false, true),
            row("python:subtle_flip", 0.94, 2, false, true),
        ];
        let thresholds = calibrate_contradiction_thresholds(
            &rows,
            ContradictionCalibrationConfig {
                min_legitimate_pass_throughput: 0.95,
                min_contradiction_recall: 0.80,
                calibrated_at_unix_ms: 1,
            },
        )
        .expect("thresholds");
        assert_eq!(thresholds.len(), 1);
        let threshold = &thresholds[0];
        assert_eq!(threshold.cell_id, "python:subtle_flip");
        assert_eq!(threshold.false_abstain_rate, 0.0);
        assert_eq!(threshold.contradiction_recall, 1.0);

        let decision = detect_multi_head_contradiction(
            Some("python:subtle_flip"),
            0.91,
            threshold.tau_failure_count,
            0,
            Some(threshold),
        )
        .expect("decision");
        assert_eq!(
            decision.kind,
            ContradictionDecisionKind::MultiHeadContradiction
        );
        assert_eq!(decision.verdict_override, Some(Verdict::Abstain));
    }
}
