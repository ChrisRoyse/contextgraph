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
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

#[derive(Debug, Default)]
pub struct SystemCostCounters {
    cuda_kernel_microseconds_total: AtomicU64,
    agent_feedback_tokens_total: AtomicU64,
    rocksdb_bytes_written_total: AtomicU64,
    rocksdb_writes_total: AtomicU64,
    heal_ticker_telemetry_total: Mutex<BTreeMap<String, HealTickerTelemetryTotals>>,
    heal_scheduler_restart_count_total: AtomicU64,
    operator_override_sampler_applied_count_total: AtomicU64,
    online_reward_signals_applied_count_total: AtomicU64,
    ewc_violations_total: AtomicU64,
    dormant_units_reinit_total: AtomicU64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SystemCostSnapshot {
    pub cuda_kernel_microseconds_total: u64,
    pub agent_feedback_tokens_total: u64,
    pub rocksdb_bytes_written_total: u64,
    pub rocksdb_writes_total: u64,
    pub heal_ticker_telemetry_total: HealTickerTelemetrySnapshot,
    pub operator_override_sampler_applied_count_total: u64,
    pub online_reward_signals_applied_count_total: u64,
    pub ewc_violations_total: u64,
    pub dormant_units_reinit_total: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SystemCostBreakdown {
    pub window_delta: SystemCostSnapshot,
    pub no_spend_caps: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct HealTickerTelemetrySnapshot {
    pub ticker_totals: BTreeMap<String, HealTickerTelemetryTotals>,
    pub scheduler_restart_count_total: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct HealTickerTelemetryTotals {
    pub run_count_total: u64,
    pub wall_clock_ms_total: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct HealTickerWindowSummary {
    pub scheduler_restart_count: u64,
    pub tickers: Vec<HealTickerWindowRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct HealTickerWindowRow {
    pub ticker: String,
    pub run_count: u64,
    pub wall_clock_ms: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum SystemCostError {
    #[error("MEJEPA_SYSTEM_COST_WEEKLY_REPORT_NOT_OBJECT: weekly report must be a JSON object")]
    WeeklyReportNotObject,
    #[error("MEJEPA_SYSTEM_COST_FORBIDDEN_FIELD: forbidden cost-cap/spend field present: {0}")]
    ForbiddenField(&'static str),
    #[error("MEJEPA_SYSTEM_COST_INVALID_TICKER: {0}")]
    InvalidTicker(String),
    #[error("MEJEPA_SYSTEM_COST_SERIALIZE_FAILED: {0}")]
    Serialize(String),
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
            heal_ticker_telemetry_total: self
                .heal_ticker_telemetry_total
                .delta(&earlier.heal_ticker_telemetry_total),
            operator_override_sampler_applied_count_total: self
                .operator_override_sampler_applied_count_total
                .saturating_sub(earlier.operator_override_sampler_applied_count_total),
            online_reward_signals_applied_count_total: self
                .online_reward_signals_applied_count_total
                .saturating_sub(earlier.online_reward_signals_applied_count_total),
            ewc_violations_total: self
                .ewc_violations_total
                .saturating_sub(earlier.ewc_violations_total),
            dormant_units_reinit_total: self
                .dormant_units_reinit_total
                .saturating_sub(earlier.dormant_units_reinit_total),
        }
    }
}

impl SystemCostBreakdown {
    pub fn from_snapshots(earlier: &SystemCostSnapshot, current: &SystemCostSnapshot) -> Self {
        Self {
            window_delta: current.delta(earlier),
            no_spend_caps: true,
        }
    }

    pub fn heal_tickers_per_window(&self) -> HealTickerWindowSummary {
        self.window_delta
            .heal_ticker_telemetry_total
            .window_summary()
    }
}

impl HealTickerTelemetrySnapshot {
    pub fn delta(&self, earlier: &Self) -> Self {
        let mut ticker_totals = BTreeMap::new();
        for (ticker, current) in &self.ticker_totals {
            let previous = earlier
                .ticker_totals
                .get(ticker)
                .cloned()
                .unwrap_or_default();
            ticker_totals.insert(
                ticker.clone(),
                HealTickerTelemetryTotals {
                    run_count_total: current
                        .run_count_total
                        .saturating_sub(previous.run_count_total),
                    wall_clock_ms_total: current
                        .wall_clock_ms_total
                        .saturating_sub(previous.wall_clock_ms_total),
                },
            );
        }
        Self {
            ticker_totals,
            scheduler_restart_count_total: self
                .scheduler_restart_count_total
                .saturating_sub(earlier.scheduler_restart_count_total),
        }
    }

    pub fn window_summary(&self) -> HealTickerWindowSummary {
        HealTickerWindowSummary {
            scheduler_restart_count: self.scheduler_restart_count_total,
            tickers: self
                .ticker_totals
                .iter()
                .map(|(ticker, totals)| HealTickerWindowRow {
                    ticker: ticker.clone(),
                    run_count: totals.run_count_total,
                    wall_clock_ms: totals.wall_clock_ms_total,
                })
                .collect(),
        }
    }
}

pub fn attach_system_cost_to_weekly_report(
    mut report: Value,
    breakdown: &SystemCostBreakdown,
) -> Result<Value, SystemCostError> {
    reject_forbidden_fields(&report)?;
    let object = report
        .as_object_mut()
        .ok_or(SystemCostError::WeeklyReportNotObject)?;
    let section = serde_json::to_value(breakdown)
        .map_err(|err| SystemCostError::Serialize(err.to_string()))?;
    object.insert("systemCost".to_string(), section);
    let heal_tickers = serde_json::to_value(breakdown.heal_tickers_per_window())
        .map_err(|err| SystemCostError::Serialize(err.to_string()))?;
    object.insert("heal_tickers_per_window".to_string(), heal_tickers);
    reject_forbidden_fields(&report)?;
    Ok(report)
}

fn reject_forbidden_fields(value: &Value) -> Result<(), SystemCostError> {
    let Some(object) = value.as_object() else {
        return Ok(());
    };
    reject_forbidden_object_fields(object)?;
    for child in object.values() {
        reject_forbidden_fields(child)?;
    }
    Ok(())
}

fn reject_forbidden_object_fields(object: &Map<String, Value>) -> Result<(), SystemCostError> {
    for forbidden in [
        "costEstimateUsd",
        "cost_estimate_usd",
        "costCapReached",
        "cost_cap_reached",
        "spendCap",
        "spend_cap",
    ] {
        if object.contains_key(forbidden) {
            return Err(SystemCostError::ForbiddenField(forbidden));
        }
    }
    Ok(())
}

impl SystemCostCounters {
    pub fn new() -> Self {
        Self::default()
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
        self.record_rocksdb_writes(bytes, 1);
    }

    pub fn record_rocksdb_writes(&self, bytes: u64, writes: u64) {
        self.rocksdb_bytes_written_total
            .fetch_add(bytes, Ordering::Relaxed);
        self.rocksdb_writes_total
            .fetch_add(writes, Ordering::Relaxed);
    }

    pub fn record_heal_ticker_run(
        &self,
        ticker: &str,
        wall_clock_ms: u64,
    ) -> Result<(), SystemCostError> {
        validate_ticker_name(ticker)?;
        let mut tickers = self
            .heal_ticker_telemetry_total
            .lock()
            .expect("heal ticker telemetry mutex poisoned");
        let entry = tickers.entry(ticker.to_string()).or_default();
        entry.run_count_total = entry.run_count_total.saturating_add(1);
        entry.wall_clock_ms_total = entry.wall_clock_ms_total.saturating_add(wall_clock_ms);
        Ok(())
    }

    pub fn record_heal_scheduler_restart(&self) {
        self.heal_scheduler_restart_count_total
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_operator_override_sampler_applied(&self, count: u64) {
        self.operator_override_sampler_applied_count_total
            .fetch_add(count, Ordering::Relaxed);
    }

    pub fn record_online_reward_signals_applied(&self, count: u64) {
        self.online_reward_signals_applied_count_total
            .fetch_add(count, Ordering::Relaxed);
    }

    pub fn record_ewc_violation(&self) {
        self.ewc_violations_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_dormant_units_reinit(&self, count: u64) {
        self.dormant_units_reinit_total
            .fetch_add(count, Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> SystemCostSnapshot {
        SystemCostSnapshot {
            cuda_kernel_microseconds_total: self
                .cuda_kernel_microseconds_total
                .load(Ordering::Relaxed),
            agent_feedback_tokens_total: self.agent_feedback_tokens_total.load(Ordering::Relaxed),
            rocksdb_bytes_written_total: self.rocksdb_bytes_written_total.load(Ordering::Relaxed),
            rocksdb_writes_total: self.rocksdb_writes_total.load(Ordering::Relaxed),
            heal_ticker_telemetry_total: HealTickerTelemetrySnapshot {
                ticker_totals: self
                    .heal_ticker_telemetry_total
                    .lock()
                    .expect("heal ticker telemetry mutex poisoned")
                    .clone(),
                scheduler_restart_count_total: self
                    .heal_scheduler_restart_count_total
                    .load(Ordering::Relaxed),
            },
            operator_override_sampler_applied_count_total: self
                .operator_override_sampler_applied_count_total
                .load(Ordering::Relaxed),
            online_reward_signals_applied_count_total: self
                .online_reward_signals_applied_count_total
                .load(Ordering::Relaxed),
            ewc_violations_total: self.ewc_violations_total.load(Ordering::Relaxed),
            dormant_units_reinit_total: self.dormant_units_reinit_total.load(Ordering::Relaxed),
        }
    }
}

fn validate_ticker_name(ticker: &str) -> Result<(), SystemCostError> {
    if ticker.is_empty() {
        return Err(SystemCostError::InvalidTicker(
            "ticker name must be non-empty".to_string(),
        ));
    }
    if ticker.len() > 64 {
        return Err(SystemCostError::InvalidTicker(format!(
            "ticker name length {} exceeds 64",
            ticker.len()
        )));
    }
    if !ticker
        .bytes()
        .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
    {
        return Err(SystemCostError::InvalidTicker(format!(
            "ticker name must use lowercase ascii, digits, or underscore: {ticker}"
        )));
    }
    Ok(())
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
        assert_eq!(snapshot.operator_override_sampler_applied_count_total, 0);
        assert_eq!(snapshot.online_reward_signals_applied_count_total, 0);
        assert_eq!(snapshot.dormant_units_reinit_total, 0);
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
            heal_ticker_telemetry_total: HealTickerTelemetrySnapshot::default(),
            operator_override_sampler_applied_count_total: 0,
            online_reward_signals_applied_count_total: 0,
            ewc_violations_total: 0,
            dormant_units_reinit_total: 0,
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

    #[test]
    fn attaches_resource_cost_breakdown_to_weekly_report() {
        let earlier = SystemCostSnapshot {
            cuda_kernel_microseconds_total: 100,
            agent_feedback_tokens_total: 10,
            rocksdb_bytes_written_total: 1_000,
            rocksdb_writes_total: 1,
            heal_ticker_telemetry_total: HealTickerTelemetrySnapshot::default(),
            operator_override_sampler_applied_count_total: 2,
            online_reward_signals_applied_count_total: 4,
            ewc_violations_total: 4,
            dormant_units_reinit_total: 6,
        };
        let current = SystemCostSnapshot {
            cuda_kernel_microseconds_total: 250,
            agent_feedback_tokens_total: 25,
            rocksdb_bytes_written_total: 1_750,
            rocksdb_writes_total: 3,
            heal_ticker_telemetry_total: HealTickerTelemetrySnapshot::default(),
            operator_override_sampler_applied_count_total: 5,
            online_reward_signals_applied_count_total: 9,
            ewc_violations_total: 7,
            dormant_units_reinit_total: 11,
        };
        let breakdown = SystemCostBreakdown::from_snapshots(&earlier, &current);
        let report = serde_json::json!({
            "reportDate": "2026-05-13",
            "shipGatePassed": false
        });
        let extended = attach_system_cost_to_weekly_report(report, &breakdown).unwrap();
        assert_eq!(
            extended["systemCost"]["windowDelta"]["cudaKernelMicrosecondsTotal"],
            150
        );
        assert_eq!(extended["systemCost"]["noSpendCaps"], true);
        assert_eq!(
            extended["systemCost"]["windowDelta"]["operatorOverrideSamplerAppliedCountTotal"],
            3
        );
        assert_eq!(
            extended["systemCost"]["windowDelta"]["onlineRewardSignalsAppliedCountTotal"],
            5
        );
        assert_eq!(
            extended["systemCost"]["windowDelta"]["ewcViolationsTotal"],
            3
        );
        assert_eq!(
            extended["systemCost"]["windowDelta"]["dormantUnitsReinitTotal"],
            5
        );
    }

    #[test]
    fn heal_ticker_telemetry_accumulates_and_renders_weekly_table() {
        let counters = SystemCostCounters::new();
        let earlier = counters.snapshot();
        counters
            .record_heal_ticker_run("observe", 5)
            .expect("record observe");
        counters
            .record_heal_ticker_run("observe", 7)
            .expect("record observe again");
        counters
            .record_heal_ticker_run("drift_check", 11)
            .expect("record drift");
        counters.record_heal_scheduler_restart();
        let current = counters.snapshot();
        let breakdown = SystemCostBreakdown::from_snapshots(&earlier, &current);
        let summary = breakdown.heal_tickers_per_window();
        assert_eq!(summary.scheduler_restart_count, 1);
        assert_eq!(summary.tickers.len(), 2);
        assert_eq!(summary.tickers[0].ticker, "drift_check");
        assert_eq!(summary.tickers[0].run_count, 1);
        assert_eq!(summary.tickers[1].ticker, "observe");
        assert_eq!(summary.tickers[1].run_count, 2);
        assert_eq!(summary.tickers[1].wall_clock_ms, 12);

        let extended = attach_system_cost_to_weekly_report(
            serde_json::json!({"reportDate": "2026-05-13"}),
            &breakdown,
        )
        .expect("attach system cost");
        assert_eq!(
            extended["heal_tickers_per_window"]["schedulerRestartCount"],
            1
        );
        assert_eq!(
            extended["heal_tickers_per_window"]["tickers"][1]["wallClockMs"],
            12
        );
    }

    #[test]
    fn heal_ticker_rejects_invalid_names_without_mutating_snapshot() {
        let counters = SystemCostCounters::new();
        let before = counters.snapshot();
        let err = counters
            .record_heal_ticker_run("DriftCheck", 10)
            .expect_err("uppercase ticker should fail");
        assert!(matches!(err, SystemCostError::InvalidTicker(_)));
        assert_eq!(counters.snapshot(), before);
    }

    #[test]
    fn operator_override_sampler_counter_accumulates() {
        let counters = SystemCostCounters::new();
        let earlier = counters.snapshot();
        counters.record_operator_override_sampler_applied(3);
        counters.record_operator_override_sampler_applied(2);
        let delta = counters.snapshot().delta(&earlier);
        assert_eq!(delta.operator_override_sampler_applied_count_total, 5);
    }

    #[test]
    fn online_reward_signals_counter_accumulates() {
        let counters = SystemCostCounters::new();
        let earlier = counters.snapshot();
        counters.record_online_reward_signals_applied(3);
        counters.record_online_reward_signals_applied(2);
        let delta = counters.snapshot().delta(&earlier);
        assert_eq!(delta.online_reward_signals_applied_count_total, 5);
    }

    #[test]
    fn ewc_violation_counter_accumulates() {
        let counters = SystemCostCounters::new();
        let earlier = counters.snapshot();
        counters.record_ewc_violation();
        counters.record_ewc_violation();
        let delta = counters.snapshot().delta(&earlier);
        assert_eq!(delta.ewc_violations_total, 2);
    }

    #[test]
    fn dormant_unit_reinit_counter_accumulates() {
        let counters = SystemCostCounters::new();
        let earlier = counters.snapshot();
        counters.record_dormant_units_reinit(3);
        counters.record_dormant_units_reinit(4);
        let delta = counters.snapshot().delta(&earlier);
        assert_eq!(delta.dormant_units_reinit_total, 7);
    }

    #[test]
    fn forbidden_spend_cap_field_fails_closed() {
        let breakdown = SystemCostBreakdown::from_snapshots(
            &SystemCostSnapshot::default(),
            &SystemCostSnapshot::default(),
        );
        let report = serde_json::json!({
            "reportDate": "2026-05-13",
            "costCapReached": false
        });
        let err = attach_system_cost_to_weekly_report(report, &breakdown).unwrap_err();
        assert!(matches!(
            err,
            SystemCostError::ForbiddenField("costCapReached")
        ));
    }
}
