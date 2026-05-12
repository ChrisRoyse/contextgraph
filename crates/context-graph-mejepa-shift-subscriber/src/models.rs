// Inspired by ruvnet/RuVector at HEAD ef5274c2 (clean-room reimplementation).

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicI64, AtomicU64, AtomicU8, Ordering};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{Result, SubscriberError};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ShiftId(pub String);

impl ShiftId {
    pub fn parse(value: impl Into<String>) -> Result<Self> {
        let value = value.into();
        if value.len() != 23
            || !value.starts_with("01J")
            || !value[3..]
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_lowercase())
        {
            return Err(SubscriberError::invalid(
                "shift_id",
                "shift_id must match ^01J[0-9A-F]{20}$",
            ));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShiftSide {
    pub path: PathBuf,
    pub sha: [u8; 32],
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShiftEntry {
    pub shift_id: ShiftId,
    pub timestamp_unix_ns: u128,
    pub tool_name: String,
    pub tool_use_id: Option<String>,
    pub session_id: [u8; 16],
    pub subject: Value,
    pub before: Value,
    pub after: Value,
    pub delta_summary: Value,
    pub verification: Value,
    pub harness_transition_path: Option<String>,
    pub byte_offset: u64,
    pub next_byte_offset: u64,
    pub source_log_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkipReason {
    AlreadyConsumed,
    NoOracleSignal,
    LStepBelowThreshold,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum ShiftOutcome {
    Predicted {
        prediction_id: String,
        observed: bool,
        dda_signal_count: usize,
        watermark_key: String,
        watermark_offset: u64,
        latency_ms: u64,
    },
    Skipped {
        reason: SkipReason,
        watermark_key: Option<String>,
        watermark_offset: Option<u64>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WatermarkRecord {
    pub session_id: String,
    pub last_consumed_shift_id: String,
    pub last_consumed_byte_offset: u64,
    pub last_advanced_at_unix_seconds: i64,
    pub producer_tool_name: Option<String>,
    pub source_log_path: Option<String>,
}

impl WatermarkRecord {
    pub fn validate(&self) -> Result<()> {
        ShiftId::parse(self.last_consumed_shift_id.clone())?;
        if decode_session_hex32(&self.session_id)?
            .iter()
            .all(|byte| *byte == 0)
        {
            return Err(SubscriberError::invalid(
                "watermark.session_id",
                "session_id must be non-zero",
            ));
        }
        if self.last_advanced_at_unix_seconds <= 0 {
            return Err(SubscriberError::invalid(
                "watermark.last_advanced_at_unix_seconds",
                "timestamp must be positive",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UtmlFactorBundle {
    pub l_step: f32,
    pub delta_p: f32,
    pub delta_k: f32,
    pub delta_omega: f32,
    pub delta_xi: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaptureAuditBundle {
    pub attempt_id: String,
    pub panels: Vec<Value>,
    pub predictions: Vec<Value>,
    pub oracle_examples: Vec<Value>,
    pub train_certs: Vec<Value>,
    pub missing_slots: Vec<String>,
    pub source_of_truth: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PanicInfo {
    pub message: String,
    pub at_unix_seconds: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LiveSubscriberStatus {
    pub subscriber_running: bool,
    pub task_alive_since: Option<i64>,
    pub processed_count: u64,
    pub observed_count: u64,
    pub dropped_l_step_below_threshold_count: u64,
    pub lag_alert_active: bool,
    pub last_panic: Option<PanicInfo>,
    pub last_watermark_per_session: BTreeMap<String, String>,
    pub rss_bytes: Option<u64>,
}

#[derive(Default)]
pub struct SubscriberMetrics {
    processed_count: AtomicU64,
    observed_count: AtomicU64,
    dropped_l_step_below_threshold_count: AtomicU64,
    lag_alert_state: AtomicU8,
    task_alive_since_unix_seconds: AtomicI64,
    latency_samples_ms: Mutex<Vec<u64>>,
    last_panic: Mutex<Option<PanicInfo>>,
}

impl SubscriberMetrics {
    pub fn mark_task_alive_now(&self) {
        self.task_alive_since_unix_seconds
            .store(unix_now_seconds(), Ordering::Relaxed);
    }

    pub fn clear_task_alive(&self) {
        self.task_alive_since_unix_seconds
            .store(0, Ordering::Relaxed);
    }

    pub fn mark_processed(&self) {
        self.processed_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn mark_observed(&self) {
        self.observed_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn mark_l_step_dropped(&self) {
        self.dropped_l_step_below_threshold_count
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn set_lag_alert_active(&self, active: bool) {
        self.lag_alert_state
            .store(if active { 1 } else { 0 }, Ordering::Relaxed);
    }

    pub fn record_latency_ms(&self, latency_ms: u64) {
        self.latency_samples_ms
            .lock()
            .expect("subscriber latency metrics lock poisoned")
            .push(latency_ms);
    }

    pub fn record_panic(&self, message: impl Into<String>) {
        *self
            .last_panic
            .lock()
            .expect("subscriber panic metrics lock poisoned") = Some(PanicInfo {
            message: message.into(),
            at_unix_seconds: unix_now_seconds(),
        });
    }

    pub fn task_alive_since(&self) -> Option<i64> {
        match self.task_alive_since_unix_seconds.load(Ordering::Relaxed) {
            value if value > 0 => Some(value),
            _ => None,
        }
    }

    pub fn last_panic(&self) -> Option<PanicInfo> {
        self.last_panic
            .lock()
            .expect("subscriber panic metrics lock poisoned")
            .clone()
    }

    pub fn snapshot_counts(&self) -> (u64, u64, u64, bool) {
        (
            self.processed_count.load(Ordering::Relaxed),
            self.observed_count.load(Ordering::Relaxed),
            self.dropped_l_step_below_threshold_count
                .load(Ordering::Relaxed),
            self.lag_alert_state.load(Ordering::Relaxed) != 0,
        )
    }

    pub fn snapshot_latency_ms(&self) -> LatencySnapshot {
        let mut samples = self
            .latency_samples_ms
            .lock()
            .expect("subscriber latency metrics lock poisoned")
            .clone();
        if samples.is_empty() {
            return LatencySnapshot::default();
        }
        samples.sort_unstable();
        let count = samples.len();
        LatencySnapshot {
            count: count as u64,
            p50: percentile(&samples, 50),
            p95: percentile(&samples, 95),
            p99: percentile(&samples, 99),
            max: *samples.last().expect("non-empty latency samples"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct LatencySnapshot {
    pub count: u64,
    pub p50: u64,
    pub p95: u64,
    pub p99: u64,
    pub max: u64,
}

fn percentile(samples: &[u64], percentile: usize) -> u64 {
    let idx = ((samples.len() - 1) * percentile).div_ceil(100);
    samples[idx.min(samples.len() - 1)]
}

fn unix_now_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time before unix epoch")
        .as_secs() as i64
}

pub fn decode_session_hex32(value: &str) -> Result<[u8; 16]> {
    if value.len() != 32 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(SubscriberError::invalid(
            "session_id",
            "session_id must be exactly 32 hexadecimal characters",
        ));
    }
    let mut out = [0u8; 16];
    hex::decode_to_slice(value, &mut out).map_err(|err| {
        SubscriberError::invalid("session_id", format!("hex decode failed: {err}"))
    })?;
    if out.iter().all(|byte| *byte == 0) {
        return Err(SubscriberError::invalid(
            "session_id",
            "session_id must be non-zero",
        ));
    }
    Ok(out)
}

pub fn encode_session_hex(session_id: [u8; 16]) -> String {
    hex::encode(session_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metrics_publish_liveness_latency_and_panic_state() {
        let metrics = SubscriberMetrics::default();
        assert_eq!(metrics.task_alive_since(), None);
        metrics.mark_task_alive_now();
        assert!(metrics.task_alive_since().is_some());
        metrics.record_latency_ms(10);
        metrics.record_latency_ms(20);
        metrics.record_panic("unit panic");
        let latency = metrics.snapshot_latency_ms();
        assert_eq!(latency.count, 2);
        assert_eq!(latency.max, 20);
        assert_eq!(
            metrics.last_panic().expect("panic info").message,
            "unit panic"
        );
        metrics.clear_task_alive();
        assert_eq!(metrics.task_alive_since(), None);
    }
}
