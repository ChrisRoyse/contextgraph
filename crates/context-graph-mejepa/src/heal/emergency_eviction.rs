use serde::{Deserialize, Serialize};

use crate::heal::errors::HealError;
use crate::heal::policy::{persist_policy_record, policy_key, scan_policy_records};
use crate::heal::store::HealRocksStore;

const EMERGENCY_EVICTION_PREFIX: &[u8] = b"phase_e/emergency-eviction/";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EmergencyEvictionAction {
    BelowThreshold,
    ThresholdReached,
    EvictionRun,
    SuppressedRepeat,
    UnrecoverableAlert,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct EmergencyEvictionDecision {
    pub total_used_bytes_before: u64,
    pub total_used_bytes_after: Option<u64>,
    pub threshold_bytes: u64,
    pub action: EmergencyEvictionAction,
    pub recorded_at_unix_ms: i64,
    pub source_of_truth_cf: String,
}

pub fn persist_emergency_eviction_decision(
    storage: &HealRocksStore,
    decision: &EmergencyEvictionDecision,
) -> Result<Vec<u8>, HealError> {
    if decision.threshold_bytes == 0 {
        return Err(HealError::invalid(
            "emergency_eviction.threshold_bytes",
            "threshold must be greater than zero",
        ));
    }
    let key = policy_key(&[
        "phase_e",
        "emergency-eviction",
        &format!(
            "{:020}-{}",
            decision.recorded_at_unix_ms, decision.total_used_bytes_before
        ),
    ])?;
    persist_policy_record(storage, &key, decision)?;
    Ok(key)
}

pub fn latest_emergency_eviction_decision(
    storage: &HealRocksStore,
) -> Result<Option<EmergencyEvictionDecision>, HealError> {
    Ok(
        scan_policy_records::<EmergencyEvictionDecision>(storage, EMERGENCY_EVICTION_PREFIX)?
            .into_iter()
            .last()
            .map(|(_, value)| value),
    )
}

pub fn classify_emergency_eviction(
    used_before: u64,
    used_after: Option<u64>,
    threshold: u64,
    previous: Option<&EmergencyEvictionDecision>,
) -> Result<EmergencyEvictionAction, HealError> {
    if threshold == 0 {
        return Err(HealError::invalid(
            "emergency_eviction.threshold",
            "threshold must be greater than zero",
        ));
    }
    if used_before < threshold {
        return Ok(EmergencyEvictionAction::BelowThreshold);
    }
    if previous
        .map(|prior| {
            prior.total_used_bytes_before == used_before
                && matches!(
                    prior.action,
                    EmergencyEvictionAction::EvictionRun
                        | EmergencyEvictionAction::ThresholdReached
                        | EmergencyEvictionAction::SuppressedRepeat
                        | EmergencyEvictionAction::UnrecoverableAlert
                )
        })
        .unwrap_or(false)
    {
        return Ok(EmergencyEvictionAction::SuppressedRepeat);
    }
    match used_after {
        None if used_before == threshold => Ok(EmergencyEvictionAction::ThresholdReached),
        None => Ok(EmergencyEvictionAction::EvictionRun),
        Some(after) if after >= used_before => Ok(EmergencyEvictionAction::UnrecoverableAlert),
        Some(after) if after >= threshold => Ok(EmergencyEvictionAction::UnrecoverableAlert),
        Some(_) => Ok(EmergencyEvictionAction::EvictionRun),
    }
}
