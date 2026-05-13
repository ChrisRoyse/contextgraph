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
    Ok(scan_policy_records::<OperatorAlert>(storage, OPERATOR_ALERT_PREFIX)?
        .into_iter()
        .map(|(_, alert)| alert)
        .collect())
}
