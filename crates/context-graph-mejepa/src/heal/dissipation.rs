use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::heal::errors::HealError;
use crate::heal::policy::{persist_policy_record, policy_key, scan_policy_records};
use crate::heal::store::HealRocksStore;
use crate::types::HeadId;

const DISSIPATION_SIGNAL_PREFIX: &[u8] = b"phase_e/dissipation-signal/";
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DissipationSignal {
    pub head: HeadId,
    pub l_step: f32,
    pub delta_k: f32,
    pub delta_xi: f32,
    pub delta_m: f32,
    pub observed_at_unix_ms: i64,
    pub source: String,
}

impl DissipationSignal {
    pub fn try_new(
        head: HeadId,
        l_step: f32,
        delta_k: f32,
        delta_xi: f32,
        delta_m: f32,
        observed_at_unix_ms: i64,
        source: impl Into<String>,
    ) -> Result<Self, HealError> {
        for (name, value) in [
            ("l_step", l_step),
            ("delta_k", delta_k),
            ("delta_xi", delta_xi),
            ("delta_m", delta_m),
        ] {
            if !value.is_finite() {
                return Err(HealError::invalid(
                    format!("dissipation_signal.{name}"),
                    "value must be finite",
                ));
            }
        }
        let source = source.into();
        if source.trim().is_empty() {
            return Err(HealError::invalid(
                "dissipation_signal.source",
                "source must be non-empty",
            ));
        }
        Ok(Self {
            head,
            l_step,
            delta_k,
            delta_xi,
            delta_m,
            observed_at_unix_ms,
            source,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DissipationConfig {
    pub window_size: usize,
    pub mean_l_step_threshold: f32,
    pub delta_m_ratio_threshold: f32,
}

impl Default for DissipationConfig {
    fn default() -> Self {
        Self {
            window_size: 1000,
            mean_l_step_threshold: 0.3,
            delta_m_ratio_threshold: 0.2,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DissipationHeadReport {
    pub head: HeadId,
    pub sample_count: usize,
    pub mean_l_step: f32,
    pub mean_delta_m: f32,
    pub delta_m_over_l_step: f32,
    pub delta_k_degrading: bool,
    pub delta_xi_degrading: bool,
    pub dissipating: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DissipationReport {
    pub generated_at_unix_ms: i64,
    pub source_of_truth_cf: String,
    pub source_signal_count: usize,
    pub global_dissipating: bool,
    pub per_head: BTreeMap<String, DissipationHeadReport>,
    pub queued_interventions: Vec<String>,
}

pub fn persist_dissipation_signal(
    storage: &HealRocksStore,
    signal: &DissipationSignal,
) -> Result<Vec<u8>, HealError> {
    let key = dissipation_signal_key(signal)?;
    persist_policy_record(storage, &key, signal)?;
    Ok(key)
}

pub fn detect_dissipation(
    storage: &HealRocksStore,
    config: &DissipationConfig,
) -> Result<DissipationReport, HealError> {
    if config.window_size == 0 {
        return Err(HealError::invalid(
            "dissipation.window_size",
            "window size must be greater than zero",
        ));
    }
    let mut records = scan_policy_records::<DissipationSignal>(storage, DISSIPATION_SIGNAL_PREFIX)?;
    if records.len() > config.window_size {
        records.drain(0..records.len() - config.window_size);
    }
    let signals = records
        .into_iter()
        .map(|(_, signal)| signal)
        .collect::<Vec<_>>();
    let mut by_head = BTreeMap::<HeadId, Vec<DissipationSignal>>::new();
    for signal in &signals {
        by_head.entry(signal.head).or_default().push(signal.clone());
    }
    let mut per_head = BTreeMap::new();
    let mut queued = Vec::new();
    let mut global = false;
    for (head, window) in by_head {
        let report = report_for_head(head, &window, config)?;
        if report.dissipating {
            global = true;
            queued.push(format!("replay_enrichment::{}", head.as_str()));
            queued.push(format!("ewc_reanchor::{}", head.as_str()));
            queued.push(format!("lr_reset::{}", head.as_str()));
        }
        per_head.insert(head.as_str().to_string(), report);
    }
    let report = DissipationReport {
        generated_at_unix_ms: chrono::Utc::now().timestamp_millis(),
        source_of_truth_cf: context_graph_mejepa_cf::CF_MEJEPA_MODEL_PROMOTIONS.to_string(),
        source_signal_count: signals.len(),
        global_dissipating: global,
        per_head,
        queued_interventions: queued,
    };
    let key = policy_key(&[
        "phase_e",
        "dissipation-report",
        &format!("{:020}", report.generated_at_unix_ms),
    ])?;
    persist_policy_record(storage, &key, &report)?;
    Ok(report)
}

fn report_for_head(
    head: HeadId,
    window: &[DissipationSignal],
    config: &DissipationConfig,
) -> Result<DissipationHeadReport, HealError> {
    if window.is_empty() {
        return Err(HealError::invalid(
            "dissipation.window",
            "head window must be non-empty",
        ));
    }
    let n = window.len() as f32;
    let mean_l_step = window.iter().map(|s| s.l_step).sum::<f32>() / n;
    let mean_delta_m = window.iter().map(|s| s.delta_m).sum::<f32>() / n;
    let ratio = if mean_l_step.abs() <= f32::EPSILON {
        0.0
    } else {
        mean_delta_m / mean_l_step
    };
    let first = window.first().expect("non-empty");
    let last = window.last().expect("non-empty");
    let delta_k_degrading = last.delta_k < first.delta_k;
    let delta_xi_degrading = last.delta_xi < first.delta_xi;
    let dissipating = mean_l_step > config.mean_l_step_threshold
        && (delta_k_degrading || delta_xi_degrading)
        && ratio < config.delta_m_ratio_threshold;
    Ok(DissipationHeadReport {
        head,
        sample_count: window.len(),
        mean_l_step,
        mean_delta_m,
        delta_m_over_l_step: ratio,
        delta_k_degrading,
        delta_xi_degrading,
        dissipating,
    })
}

fn dissipation_signal_key(signal: &DissipationSignal) -> Result<Vec<u8>, HealError> {
    let mut hasher = Sha256::new();
    hasher.update(signal.head.as_str());
    hasher.update(signal.l_step.to_le_bytes());
    hasher.update(signal.delta_k.to_le_bytes());
    hasher.update(signal.delta_xi.to_le_bytes());
    hasher.update(signal.delta_m.to_le_bytes());
    hasher.update(signal.observed_at_unix_ms.to_be_bytes());
    hasher.update(signal.source.as_bytes());
    policy_key(&[
        "phase_e",
        "dissipation-signal",
        &format!(
            "{:020}-{}",
            signal.observed_at_unix_ms,
            hex::encode(hasher.finalize())
        ),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::heal::store::HealRocksStore;

    #[test]
    fn high_l_step_low_delta_m_with_degrading_xi_detects() {
        let temp = tempfile::tempdir().unwrap();
        let storage = HealRocksStore::open(temp.path().join("db")).unwrap();
        for idx in 0..4 {
            persist_dissipation_signal(
                storage.as_ref(),
                &DissipationSignal::try_new(
                    HeadId::Oracle,
                    0.4,
                    0.8,
                    0.8 - idx as f32 * 0.1,
                    0.02,
                    1_778_000_000_000 + idx,
                    "unit-test-real-rocksdb",
                )
                .unwrap(),
            )
            .unwrap();
        }
        let report = detect_dissipation(storage.as_ref(), &DissipationConfig::default()).unwrap();
        assert!(report.global_dissipating);
        assert!(!report.queued_interventions.is_empty());
    }
}
