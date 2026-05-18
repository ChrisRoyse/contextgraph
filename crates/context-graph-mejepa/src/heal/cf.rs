// RocksDB CF descriptors for SELF-HEALING domain (per TECH-SELF-HEALING <rocksdb_column_families>).
// Two CFs (CF_MEJEPA_WEIGHT_BLOBS, CF_MEJEPA_SHIFT_WATERMARK) are already-shipped
// cross-domain names and are re-exported by name only.

use serde::{Deserialize, Serialize};
use sha2::Digest;

use crate::heal::drift::{DriftSample, DriftSeverity};
use crate::heal::errors::HealError;

pub use context_graph_mejepa_cf::{
    CF_MEJEPA_ACTIVE_POINTERS, CF_MEJEPA_CALIBRATION_HISTORY, CF_MEJEPA_CHUNK_DEPENDENCY_GRAPH,
    CF_MEJEPA_CHUNK_FOUNDATIONALITY, CF_MEJEPA_DISTILL_STEPS, CF_MEJEPA_DRIFT_HISTORY,
    CF_MEJEPA_DRIFT_WINDOW, CF_MEJEPA_FISHER_SNAPSHOTS, CF_MEJEPA_HEAL_REPORTS,
    CF_MEJEPA_MODEL_PROMOTIONS, CF_MEJEPA_PLASTICITY_HISTORY, CF_MEJEPA_PRODUCTION_TELEMETRY,
    CF_MEJEPA_SHIFT_WATERMARK, CF_MEJEPA_TRAIN_CERTS, CF_MEJEPA_WEIGHT_BLOBS,
};

pub fn all_self_healing_cf_names() -> [&'static str; 7] {
    context_graph_mejepa_cf::SELF_HEALING_CFS
        .try_into()
        .expect("SELF_HEALING_CFS length is fixed at 7")
}

pub fn all_referenced_cf_names() -> Vec<&'static str> {
    let mut names = context_graph_mejepa_cf::SELF_HEALING_REFERENCED_CFS.to_vec();
    for cf in [
        context_graph_mejepa_cf::CF_MEJEPA_EVAL_REPORTS,
        context_graph_mejepa_cf::CF_MEJEPA_MODEL_PROMOTIONS,
        context_graph_mejepa_cf::CF_MEJEPA_PRODUCTION_TELEMETRY,
        context_graph_mejepa_cf::CF_MEJEPA_CONSTELLATION,
        context_graph_mejepa_cf::CF_MEJEPA_CONSTELLATION_REFRESH_REPORTS,
        context_graph_mejepa_cf::CF_MEJEPA_CONSTELLATION_REFRESH_LOG,
        context_graph_mejepa_cf::CF_MEJEPA_QUOTA_STATE,
        context_graph_mejepa_cf::CF_MEJEPA_COLD_CELL_METRICS,
        context_graph_mejepa_cf::CF_MEJEPA_CHUNK_DEPENDENCY_GRAPH,
        context_graph_mejepa_cf::CF_MEJEPA_CHUNK_FOUNDATIONALITY,
        context_graph_mejepa_cf::CF_MEJEPA_ACTIVE_LEARNING_QUEUE,
    ] {
        if !names.contains(&cf) {
            names.push(cf);
        }
    }
    names
}

pub fn encode_fisher_snapshot_key(boundary_step: u64) -> [u8; 8] {
    boundary_step.to_be_bytes()
}

pub fn encode_plasticity_history_key(training_tick: u64) -> [u8; 8] {
    training_tick.to_be_bytes()
}

pub fn decode_fisher_snapshot_key(bytes: &[u8]) -> Result<u64, HealError> {
    if bytes.len() != 8 {
        return Err(HealError::invalid(
            "fisher_snapshot_key",
            format!("expected 8 bytes, got {}", bytes.len()),
        ));
    }
    let mut out = [0u8; 8];
    out.copy_from_slice(bytes);
    Ok(u64::from_be_bytes(out))
}

pub fn encode_drift_window_key(ring_buffer_offset: u64) -> [u8; 8] {
    ring_buffer_offset.to_be_bytes()
}

pub fn encode_drift_history_key(timestamp_seconds: i64) -> [u8; 8] {
    timestamp_seconds.to_be_bytes()
}

pub fn encode_heal_report_key(timestamp_seconds: i64) -> [u8; 8] {
    timestamp_seconds.to_be_bytes()
}

pub fn encode_holdout_rotation_event_key(rotation_index: u64, timestamp_millis: i64) -> Vec<u8> {
    format!("holdout-rotation/{rotation_index:016x}/{timestamp_millis:020}").into_bytes()
}

pub fn is_holdout_rotation_event_key(key: &[u8]) -> bool {
    key.starts_with(b"holdout-rotation/")
}

pub fn encode_distill_step_key(timestamp_nanos: i64) -> [u8; 8] {
    timestamp_nanos.to_be_bytes()
}

pub fn encode_active_pointer_key(name: &str) -> Result<Vec<u8>, HealError> {
    validate_active_pointer_name(name)?;
    Ok(name.as_bytes().to_vec())
}

pub fn encode_calibration_history_key(frozen_at_seconds: i64) -> [u8; 8] {
    frozen_at_seconds.to_be_bytes()
}

pub fn encode_calibration_history_record_key(frozen_at_seconds: i64, version: &str) -> Vec<u8> {
    let mut key = Vec::with_capacity(40);
    key.extend_from_slice(&frozen_at_seconds.to_be_bytes());
    key.extend_from_slice(&sha2::Sha256::digest(version.as_bytes())[..16]);
    key
}

pub fn validate_active_pointer_name(name: &str) -> Result<(), HealError> {
    match name {
        "active_weights" | "active_calibration" | "active_constellation" | "witness_quarantine" => {
            Ok(())
        }
        _ => Err(HealError::invalid(
            "active_pointer.name",
            format!("unsupported active pointer key {name:?}"),
        )),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ActivePointerValue {
    pub theta_sha_or_version: Vec<u8>,
    pub frozen_at: i64,
}

impl ActivePointerValue {
    pub fn try_new(theta_sha_or_version: Vec<u8>, frozen_at: i64) -> Result<Self, HealError> {
        if theta_sha_or_version.is_empty() {
            return Err(HealError::invalid(
                "active_pointer.value",
                "theta/version bytes must be non-empty",
            ));
        }
        Ok(Self {
            theta_sha_or_version,
            frozen_at,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DriftHistoryRecord {
    pub severity: DriftSeverity,
    pub empirical_coverage: f32,
    pub window_start_offset: u64,
    pub window_end_offset: u64,
}

impl DriftHistoryRecord {
    pub fn from_sample_window(
        severity: DriftSeverity,
        empirical_coverage: f32,
        window: &[DriftSample],
    ) -> Result<Self, HealError> {
        if window.is_empty() {
            return Err(HealError::invalid(
                "drift_history.window",
                "window must be non-empty",
            ));
        }
        Ok(Self {
            severity,
            empirical_coverage,
            window_start_offset: window[0].witness_chain_offset,
            window_end_offset: window[window.len() - 1].witness_chain_offset,
        })
    }
}

pub fn encode_value<T: Serialize>(value: &T) -> Result<Vec<u8>, HealError> {
    Ok(bincode::serialize(value)?)
}

pub fn decode_value<T: for<'de> Deserialize<'de>>(bytes: &[u8]) -> Result<T, HealError> {
    Ok(bincode::deserialize(bytes)?)
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RetentionPolicy {
    pub time_capped_days: Option<u32>,
    pub count_capped: Option<u64>,
    pub byte_capped_gb: Option<u32>,
    pub single_row_per_key: bool,
}

pub fn cf_retention_descriptor(cf_name: &str) -> Option<RetentionPolicy> {
    match cf_name {
        CF_MEJEPA_FISHER_SNAPSHOTS => Some(RetentionPolicy {
            time_capped_days: None,
            count_capped: Some(5),
            byte_capped_gb: None,
            single_row_per_key: false,
        }),
        CF_MEJEPA_DRIFT_WINDOW => Some(RetentionPolicy {
            time_capped_days: None,
            count_capped: Some(1000),
            byte_capped_gb: None,
            single_row_per_key: false,
        }),
        CF_MEJEPA_DRIFT_HISTORY => Some(RetentionPolicy {
            time_capped_days: Some(90),
            count_capped: None,
            byte_capped_gb: None,
            single_row_per_key: false,
        }),
        CF_MEJEPA_HEAL_REPORTS => Some(RetentionPolicy {
            time_capped_days: Some(365),
            count_capped: None,
            byte_capped_gb: None,
            single_row_per_key: false,
        }),
        CF_MEJEPA_DISTILL_STEPS => Some(RetentionPolicy {
            time_capped_days: Some(30),
            count_capped: None,
            byte_capped_gb: None,
            single_row_per_key: false,
        }),
        CF_MEJEPA_ACTIVE_POINTERS => Some(RetentionPolicy {
            time_capped_days: None,
            count_capped: Some(4),
            byte_capped_gb: None,
            single_row_per_key: true,
        }),
        CF_MEJEPA_CALIBRATION_HISTORY => Some(RetentionPolicy {
            time_capped_days: None,
            count_capped: Some(30),
            byte_capped_gb: None,
            single_row_per_key: false,
        }),
        CF_MEJEPA_PLASTICITY_HISTORY => Some(RetentionPolicy {
            time_capped_days: Some(365),
            count_capped: None,
            byte_capped_gb: None,
            single_row_per_key: false,
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_cf_names_match_prefix() {
        for cf in all_self_healing_cf_names() {
            assert!(cf.starts_with("CF_MEJEPA_"));
            assert_eq!(cf, cf.to_ascii_uppercase());
        }
    }

    #[test]
    fn fisher_snapshot_key_is_big_endian_and_decodes() {
        let key = encode_fisher_snapshot_key(258);
        assert_eq!(key, [0, 0, 0, 0, 0, 0, 1, 2]);
        assert_eq!(decode_fisher_snapshot_key(&key).unwrap(), 258);
        assert!(decode_fisher_snapshot_key(&key[..7]).is_err());
    }

    #[test]
    fn active_pointer_names_are_strict() {
        assert!(encode_active_pointer_key("active_weights").is_ok());
        assert!(encode_active_pointer_key("witness_quarantine").is_ok());
        assert!(encode_active_pointer_key("other").is_err());
    }

    #[test]
    fn referenced_cfs_include_weight_and_watermark() {
        let names = all_referenced_cf_names();
        assert!(names.contains(&CF_MEJEPA_WEIGHT_BLOBS));
        assert!(names.contains(&CF_MEJEPA_SHIFT_WATERMARK));
        assert!(names.contains(&CF_MEJEPA_MODEL_PROMOTIONS));
        assert!(names.contains(&CF_MEJEPA_PLASTICITY_HISTORY));
        assert!(names.contains(&context_graph_mejepa_cf::CF_MEJEPA_CONSTELLATION_REFRESH_LOG));
        assert!(names.contains(&CF_MEJEPA_CHUNK_DEPENDENCY_GRAPH));
        assert!(names.contains(&CF_MEJEPA_CHUNK_FOUNDATIONALITY));
        assert!(names.contains(&context_graph_mejepa_cf::CF_MEJEPA_ACTIVE_LEARNING_QUEUE));
        assert_eq!(
            names.len(),
            context_graph_mejepa_cf::SELF_HEALING_REFERENCED_CFS.len() + 11
        );
    }
}
