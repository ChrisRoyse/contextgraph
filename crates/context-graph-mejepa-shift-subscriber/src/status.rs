// Inspired by ruvnet/RuVector at HEAD ef5274c2 (clean-room reimplementation).

use std::collections::BTreeMap;

use context_graph_mejepa_embedders::cache::EmbedderCache;
use serde_json::{json, Value};

use crate::{Result, ShiftSubscriber};

impl ShiftSubscriber {
    pub fn capture_minimal_status(&self) -> Result<Value> {
        let metrics = self.metrics();
        let (processed, observed, dropped, lag_alert_active) = metrics.snapshot_counts();
        let task_alive_since = metrics.task_alive_since();
        let watermarks = self.watermark_writer.read_all()?;
        let last_watermark_per_session = watermarks
            .iter()
            .map(|(session, record)| (session.clone(), record.last_consumed_shift_id.clone()))
            .collect::<BTreeMap<_, _>>();
        Ok(json!({
            "subscriber_running": task_alive_since.is_some(),
            "task_alive_since": task_alive_since,
            "processed_count": processed,
            "observed_count": observed,
            "dropped_l_step_below_threshold_count": dropped,
            "lag_alert_active": lag_alert_active,
            "latency_ms": metrics.snapshot_latency_ms(),
            "last_panic": metrics.last_panic(),
            "embedder_cache": self.embedder_cache_status()?,
            "last_watermark_per_session": last_watermark_per_session
        }))
    }

    fn embedder_cache_status(&self) -> Result<Value> {
        let cache = EmbedderCache::new(self.dda_cache_root()?)?;
        let telemetry_path = cache.telemetry_path();
        let telemetry = if telemetry_path.exists() {
            Some(serde_json::to_value(cache.telemetry()?)?)
        } else {
            None
        };
        Ok(json!({
            "telemetry_path": telemetry_path,
            "telemetry_present": telemetry.is_some(),
            "telemetry": telemetry
        }))
    }
}
