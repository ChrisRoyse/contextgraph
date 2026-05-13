//! TASK-OBS-013 — System cost telemetry (resource counters, NO spend caps).
//!
//! Per `CLAUDE.md §3.11` and `feedback_no_cost_caps`, ME-JEPA never
//! tracks dollar amounts and never enforces per-turn / per-day budgets.
//! This module records *resource* counters that the weekly eval report
//! can surface so the operator sees system load over time:
//!
//! * `cuda_kernel_microseconds_total` — accumulated CUDA cycles.
//! * `agent_feedback_tokens_total` — sum of operator-supplied token
//!   counts on `mejepa_record_agent_feedback` payloads.
//! * `rocksdb_bytes_written_total` — bytes written to inference CFs.
//! * `rocksdb_writes_total` — count of write ops.
//!
//! All counters are monotonic; the weekly window is computed by the
//! report renderer as `current_value - snapshot_at_window_start`. There
//! is NO `cost_estimate_usd` field, and NO `cost_cap_reached` status.

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Default)]
pub struct SystemCostCounters {
    cuda_kernel_microseconds_total: AtomicU64,
    agent_feedback_tokens_total: AtomicU64,
    rocksdb_bytes_written_total: AtomicU64,
    rocksdb_writes_total: AtomicU64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SystemCostSnapshot {
    pub cuda_kernel_microseconds_total: u64,
    pub agent_feedback_tokens_total: u64,
    pub rocksdb_bytes_written_total: u64,
    pub rocksdb_writes_total: u64,
}

impl SystemCostSnapshot {
    pub fn delta(&self, earlier: &Self) -> Self {
        Self {
            cuda_kernel_microseconds_total: self
                .cuda_kernel_microseconds_total
                .saturating_sub(earlier.cuda_kernel_microseconds_total),
            agent_feedback_tokens_total: self
                .agent_feedback_tokens_total
                .saturating_sub(earlier.agent_feedback_tokens_total),
            rocksdb_bytes_written_total: self
                .rocksdb_bytes_written_total
                .saturating_sub(earlier.rocksdb_bytes_written_total),
            rocksdb_writes_total: self
                .rocksdb_writes_total
                .saturating_sub(earlier.rocksdb_writes_total),
        }
    }
}

impl SystemCostCounters {
    pub const fn new() -> Self {
        Self {
            cuda_kernel_microseconds_total: AtomicU64::new(0),
            agent_feedback_tokens_total: AtomicU64::new(0),
            rocksdb_bytes_written_total: AtomicU64::new(0),
            rocksdb_writes_total: AtomicU64::new(0),
        }
    }

    pub fn record_cuda_microseconds(&self, micros: u64) {
        self.cuda_kernel_microseconds_total
            .fetch_add(micros, Ordering::Relaxed);
    }

    pub fn record_agent_feedback_tokens(&self, tokens: u64) {
        self.agent_feedback_tokens_total
            .fetch_add(tokens, Ordering::Relaxed);
    }

    pub fn record_rocksdb_write(&self, bytes: u64) {
        self.rocksdb_bytes_written_total
            .fetch_add(bytes, Ordering::Relaxed);
        self.rocksdb_writes_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> SystemCostSnapshot {
        SystemCostSnapshot {
            cuda_kernel_microseconds_total: self
                .cuda_kernel_microseconds_total
                .load(Ordering::Relaxed),
            agent_feedback_tokens_total: self.agent_feedback_tokens_total.load(Ordering::Relaxed),
            rocksdb_bytes_written_total: self.rocksdb_bytes_written_total.load(Ordering::Relaxed),
            rocksdb_writes_total: self.rocksdb_writes_total.load(Ordering::Relaxed),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counters_start_at_zero() {
        let counters = SystemCostCounters::new();
        let snapshot = counters.snapshot();
        assert_eq!(snapshot.cuda_kernel_microseconds_total, 0);
        assert_eq!(snapshot.rocksdb_writes_total, 0);
    }

    #[test]
    fn record_cuda_accumulates() {
        let counters = SystemCostCounters::new();
        counters.record_cuda_microseconds(1_500);
        counters.record_cuda_microseconds(2_500);
        let snapshot = counters.snapshot();
        assert_eq!(snapshot.cuda_kernel_microseconds_total, 4_000);
    }

    #[test]
    fn record_rocksdb_increments_both_counters() {
        let counters = SystemCostCounters::new();
        counters.record_rocksdb_write(123);
        counters.record_rocksdb_write(456);
        let snapshot = counters.snapshot();
        assert_eq!(snapshot.rocksdb_bytes_written_total, 579);
        assert_eq!(snapshot.rocksdb_writes_total, 2);
    }

    #[test]
    fn delta_against_earlier_snapshot() {
        let counters = SystemCostCounters::new();
        counters.record_agent_feedback_tokens(100);
        let earlier = counters.snapshot();
        counters.record_agent_feedback_tokens(250);
        let now = counters.snapshot();
        let delta = now.delta(&earlier);
        assert_eq!(delta.agent_feedback_tokens_total, 250);
    }

    #[test]
    fn delta_clamps_at_zero_when_counters_reset() {
        // If the daemon restarted, the snapshot can be greater than the
        // monotonic counter; saturating_sub clamps at zero so the
        // weekly report never reports a negative window.
        let counters = SystemCostCounters::new();
        let earlier = SystemCostSnapshot {
            cuda_kernel_microseconds_total: 10_000,
            agent_feedback_tokens_total: 0,
            rocksdb_bytes_written_total: 0,
            rocksdb_writes_total: 0,
        };
        let now = counters.snapshot();
        let delta = now.delta(&earlier);
        assert_eq!(delta.cuda_kernel_microseconds_total, 0);
    }

    #[test]
    fn snapshot_round_trips_through_json() {
        let counters = SystemCostCounters::new();
        counters.record_cuda_microseconds(1_000);
        counters.record_agent_feedback_tokens(2_000);
        counters.record_rocksdb_write(3_000);
        let snapshot = counters.snapshot();
        let json = serde_json::to_string(&snapshot).unwrap();
        let parsed: SystemCostSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, snapshot);
    }

    #[test]
    fn no_cost_estimate_usd_field_exists() {
        let counters = SystemCostCounters::new();
        let json = serde_json::to_string(&counters.snapshot()).unwrap();
        assert!(!json.contains("costEstimateUsd"));
        assert!(!json.contains("cost_estimate_usd"));
        assert!(!json.contains("costCapReached"));
    }
}
