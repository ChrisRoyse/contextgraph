use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::heal::drift::DriftSeverity;
use crate::heal::errors::HealError;
use crate::heal::policy::{persist_policy_record, policy_key, scan_policy_records};
use crate::heal::store::HealRocksStore;

const OPERATOR_ALERT_PREFIX: &[u8] = b"phase_e/operator-alert/";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DriftAttributionKind {
    DataShift,
    ModelStaleness,
    SpecChange,
    PerCellDistributionShift,
    UnknownWithEvidence,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DriftAttribution {
    pub kind: DriftAttributionKind,
    pub confidence: f32,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "snake_case")]
pub enum FailingCellRootCause {
    InsufficientTrainingData,
    EmbedderSignalGap,
    LabelNoise,
    OracleFlakiness,
    DistributionShift,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct FailingCellSignature {
    pub cell: String,
    pub correlation: Option<f32>,
    pub holdout_count: usize,
    pub min_holdout_count: usize,
    pub embedder_pairwise_mi: Option<f32>,
    pub blind_spot_z: Option<f32>,
    pub label_disagreement_rate: Option<f32>,
    pub oracle_flake_rate: Option<f32>,
    pub distribution_shift_score: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct FailingCellClassification {
    pub cell: String,
    pub root_cause: FailingCellRootCause,
    pub confidence: f32,
    pub heuristic: String,
    pub evidence: Vec<String>,
}

pub fn classify_failing_cell(
    signature: &FailingCellSignature,
) -> Result<FailingCellClassification, HealError> {
    validate_failing_cell_signature(signature)?;
    let (root_cause, confidence, heuristic, evidence) =
        if signature.holdout_count < signature.min_holdout_count {
            (
                FailingCellRootCause::InsufficientTrainingData,
                0.95,
                "holdout_count < min_holdout_count",
                vec![
                    format!("holdout_count={}", signature.holdout_count),
                    format!("min_holdout_count={}", signature.min_holdout_count),
                ],
            )
        } else if threshold_at_least(signature.oracle_flake_rate, 0.10) {
            let value = signature.oracle_flake_rate.ok_or_else(|| {
                HealError::invalid(
                    "failing_cell.oracle_flake_rate",
                    "oracle flake rate threshold fired without a value",
                )
            })?;
            (
                FailingCellRootCause::OracleFlakiness,
                0.90,
                "oracle_flake_rate >= 0.10",
                vec![format!("oracle_flake_rate={value:.6}")],
            )
        } else if threshold_at_least(signature.label_disagreement_rate, 0.15) {
            let value = signature.label_disagreement_rate.ok_or_else(|| {
                HealError::invalid(
                    "failing_cell.label_disagreement_rate",
                    "label disagreement threshold fired without a value",
                )
            })?;
            (
                FailingCellRootCause::LabelNoise,
                0.86,
                "label_disagreement_rate >= 0.15",
                vec![format!("label_disagreement_rate={value:.6}")],
            )
        } else if threshold_at_least(signature.distribution_shift_score, 0.30) {
            let value = signature.distribution_shift_score.ok_or_else(|| {
                HealError::invalid(
                    "failing_cell.distribution_shift_score",
                    "distribution shift threshold fired without a value",
                )
            })?;
            (
                FailingCellRootCause::DistributionShift,
                0.84,
                "distribution_shift_score >= 0.30",
                vec![format!("distribution_shift_score={value:.6}")],
            )
        } else if threshold_at_least(signature.blind_spot_z, 2.0)
            || threshold_at_most(signature.embedder_pairwise_mi, 0.15)
        {
            let mut evidence = Vec::new();
            if let Some(value) = signature.blind_spot_z {
                evidence.push(format!("blind_spot_z={value:.6}"));
            }
            if let Some(value) = signature.embedder_pairwise_mi {
                evidence.push(format!("embedder_pairwise_mi={value:.6}"));
            }
            (
                FailingCellRootCause::EmbedderSignalGap,
                0.82,
                "blind_spot_z >= 2.0 or embedder_pairwise_mi <= 0.15",
                evidence,
            )
        } else {
            let mut evidence = vec![format!("holdout_count={}", signature.holdout_count)];
            if let Some(value) = signature.correlation {
                evidence.push(format!("correlation={value:.6}"));
            }
            (
                FailingCellRootCause::Unknown,
                0.25,
                "no configured root-cause heuristic threshold fired",
                evidence,
            )
        };
    Ok(FailingCellClassification {
        cell: signature.cell.clone(),
        root_cause,
        confidence,
        heuristic: heuristic.to_string(),
        evidence,
    })
}

fn validate_failing_cell_signature(signature: &FailingCellSignature) -> Result<(), HealError> {
    if signature.cell.trim().is_empty() {
        return Err(HealError::invalid(
            "failing_cell.cell",
            "cell must be non-empty",
        ));
    }
    if signature.min_holdout_count == 0 {
        return Err(HealError::invalid(
            "failing_cell.min_holdout_count",
            "minimum holdout count must be greater than zero",
        ));
    }
    validate_optional_bounded("failing_cell.correlation", signature.correlation, -1.0, 1.0)?;
    validate_optional_bounded(
        "failing_cell.embedder_pairwise_mi",
        signature.embedder_pairwise_mi,
        0.0,
        1.0,
    )?;
    validate_optional_bounded(
        "failing_cell.label_disagreement_rate",
        signature.label_disagreement_rate,
        0.0,
        1.0,
    )?;
    validate_optional_bounded(
        "failing_cell.oracle_flake_rate",
        signature.oracle_flake_rate,
        0.0,
        1.0,
    )?;
    validate_optional_bounded(
        "failing_cell.distribution_shift_score",
        signature.distribution_shift_score,
        0.0,
        1.0,
    )?;
    if let Some(value) = signature.blind_spot_z {
        if !value.is_finite() || value < 0.0 {
            return Err(HealError::invalid(
                "failing_cell.blind_spot_z",
                "blind-spot z-score must be finite and non-negative",
            ));
        }
    }
    Ok(())
}

fn validate_optional_bounded(
    field: &'static str,
    value: Option<f32>,
    min: f32,
    max: f32,
) -> Result<(), HealError> {
    if let Some(value) = value {
        if !value.is_finite() || value < min || value > max {
            return Err(HealError::invalid(
                field,
                format!("value must be finite and in [{min},{max}]; got {value}"),
            ));
        }
    }
    Ok(())
}

fn threshold_at_least(value: Option<f32>, threshold: f32) -> bool {
    matches!(value, Some(value) if value >= threshold)
}

fn threshold_at_most(value: Option<f32>, threshold: f32) -> bool {
    matches!(value, Some(value) if value <= threshold)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct OperatorAlert {
    pub alert_id: String,
    pub severity: DriftSeverity,
    pub title: String,
    pub body: String,
    pub attribution: Option<DriftAttribution>,
    pub created_at_unix_ms: i64,
    pub source_of_truth_cf: String,
}

pub fn classify_drift_attribution(
    per_cell_only: bool,
    tau_drift_fraction: f32,
    model_age_days: u32,
    spec_change_seen: bool,
) -> Result<DriftAttribution, HealError> {
    if !tau_drift_fraction.is_finite() || tau_drift_fraction < 0.0 {
        return Err(HealError::invalid(
            "drift_attribution.tau_drift_fraction",
            "tau drift must be finite and non-negative",
        ));
    }
    let (kind, confidence, evidence) = if spec_change_seen {
        (
            DriftAttributionKind::SpecChange,
            0.90,
            vec!["spec_change_seen=true".to_string()],
        )
    } else if per_cell_only {
        (
            DriftAttributionKind::PerCellDistributionShift,
            0.85,
            vec!["one_or_more_cells_regressed_without_global_regression".to_string()],
        )
    } else if model_age_days > 90 {
        (
            DriftAttributionKind::ModelStaleness,
            0.80,
            vec![format!("model_age_days={model_age_days}")],
        )
    } else if tau_drift_fraction > 0.03 {
        (
            DriftAttributionKind::DataShift,
            0.80,
            vec![format!("tau_drift_fraction={tau_drift_fraction}")],
        )
    } else {
        (
            DriftAttributionKind::UnknownWithEvidence,
            0.25,
            vec![
                format!("tau_drift_fraction={tau_drift_fraction}"),
                format!("model_age_days={model_age_days}"),
            ],
        )
    };
    Ok(DriftAttribution {
        kind,
        confidence,
        evidence,
    })
}

pub fn write_operator_alert(
    storage: &HealRocksStore,
    severity: DriftSeverity,
    title: impl Into<String>,
    body: impl Into<String>,
    attribution: Option<DriftAttribution>,
) -> Result<OperatorAlert, HealError> {
    let title = title.into();
    let body = body.into();
    if title.trim().is_empty() || body.trim().is_empty() {
        return Err(HealError::invalid(
            "operator_alert",
            "title and body must be non-empty",
        ));
    }
    let created_at = chrono::Utc::now().timestamp_millis();
    let mut hasher = Sha256::new();
    hasher.update(&title);
    hasher.update(&body);
    hasher.update(created_at.to_be_bytes());
    let alert = OperatorAlert {
        alert_id: hex::encode(hasher.finalize()),
        severity,
        title,
        body,
        attribution,
        created_at_unix_ms: created_at,
        source_of_truth_cf: context_graph_mejepa_cf::CF_MEJEPA_MODEL_PROMOTIONS.to_string(),
    };
    let key = policy_key(&[
        "phase_e",
        "operator-alert",
        &format!("{:020}-{}", created_at, alert.alert_id),
    ])?;
    persist_policy_record(storage, &key, &alert)?;
    Ok(alert)
}

pub fn list_operator_alerts(storage: &HealRocksStore) -> Result<Vec<OperatorAlert>, HealError> {
    Ok(
        scan_policy_records::<OperatorAlert>(storage, OPERATOR_ALERT_PREFIX)?
            .into_iter()
            .map(|(_, alert)| alert)
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signature(
        cell: impl Into<String>,
        correlation: Option<f32>,
        holdout_count: usize,
        min_holdout_count: usize,
    ) -> FailingCellSignature {
        FailingCellSignature {
            cell: cell.into(),
            correlation,
            holdout_count,
            min_holdout_count,
            embedder_pairwise_mi: None,
            blind_spot_z: None,
            label_disagreement_rate: None,
            oracle_flake_rate: None,
            distribution_shift_score: None,
        }
    }

    fn classify_root(signature: FailingCellSignature) -> FailingCellRootCause {
        classify_failing_cell(&signature).unwrap().root_cause
    }

    #[test]
    fn classify_failing_cell_maps_primary_synthetic_signatures() {
        assert_eq!(
            classify_root(signature("known_good::python", Some(0.72), 4, 10)),
            FailingCellRootCause::InsufficientTrainingData
        );

        let mut embedder_gap = signature("off_by_one::rust", Some(0.81), 20, 10);
        embedder_gap.blind_spot_z = Some(2.1);
        assert_eq!(
            classify_root(embedder_gap),
            FailingCellRootCause::EmbedderSignalGap
        );

        let mut label_noise = signature("subtle_flip::go", Some(0.83), 20, 10);
        label_noise.label_disagreement_rate = Some(0.17);
        assert_eq!(classify_root(label_noise), FailingCellRootCause::LabelNoise);

        let mut distribution_shift = signature("wrong_file::ruby", Some(0.78), 20, 10);
        distribution_shift.distribution_shift_score = Some(0.31);
        assert_eq!(
            classify_root(distribution_shift),
            FailingCellRootCause::DistributionShift
        );

        assert_eq!(
            classify_root(signature("compile_error::php", Some(0.89), 20, 10)),
            FailingCellRootCause::Unknown
        );
    }

    #[test]
    fn classify_failing_cell_prioritizes_oracle_flakiness_before_label_noise() {
        let mut signature = signature("swap_variable::java", Some(0.80), 20, 10);
        signature.oracle_flake_rate = Some(0.12);
        signature.label_disagreement_rate = Some(0.25);

        let classification = classify_failing_cell(&signature).unwrap();

        assert_eq!(
            classification.root_cause,
            FailingCellRootCause::OracleFlakiness
        );
        assert_eq!(classification.heuristic, "oracle_flake_rate >= 0.10");
    }

    #[test]
    fn classify_failing_cell_fails_closed_on_invalid_signatures() {
        let err = classify_failing_cell(&signature("", Some(0.90), 20, 10)).unwrap_err();
        assert_eq!(err.code(), "MEJEPA_HEAL_INVALID_STATE");

        let mut invalid_score = signature("bad_score::python", Some(0.90), 20, 10);
        invalid_score.distribution_shift_score = Some(1.2);
        let err = classify_failing_cell(&invalid_score).unwrap_err();
        assert_eq!(err.code(), "MEJEPA_HEAL_INVALID_STATE");
    }
}
