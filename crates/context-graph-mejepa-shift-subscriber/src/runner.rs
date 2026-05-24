// Inspired by ruvnet/RuVector at HEAD ef5274c2 (clean-room reimplementation).
use crate::models::{decode_session_hex32, LatencySnapshot, ShiftEntry, ShiftOutcome};
use crate::{Result, ShiftLogTail, ShiftSubscriber, SubscriberError};
use futures_util::FutureExt;
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::panic::AssertUnwindSafe;
use std::path::Path;
use std::time::{Duration, Instant};
use tokio::sync::watch;
use tracing::error;
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubscriberRunSummary {
    pub processed: u64,
    pub observed: u64,
    pub dropped_l_step_below_threshold: u64,
    pub lag_alert_active: bool,
    pub latency_ms: LatencySnapshot,
    pub outcomes: Vec<ShiftOutcome>,
    pub status: serde_json::Value,
}
impl ShiftSubscriber {
    pub async fn run_until_idle(&self) -> Result<SubscriberRunSummary> {
        self.metrics().mark_task_alive_now();
        let result = self.run_until_idle_inner().await;
        self.metrics().clear_task_alive();
        result
    }
    async fn run_until_idle_inner(&self) -> Result<SubscriberRunSummary> {
        let mut outcomes = Vec::new();
        let mut lag_exceeded_since = None;
        loop {
            let mut tails = self.open_tails()?;
            let mut made_progress = false;
            for tail in &mut tails {
                self.update_lag_alert(tail, &mut lag_exceeded_since)?;
                match tail.poll_next_line().await {
                    Ok(Some(entry)) => {
                        let outcome = self.process_shift_supervised(&entry).await;
                        match outcome {
                            Ok(outcome) => {
                                outcomes.push(outcome);
                                made_progress = true;
                            }
                            Err(err) => {
                                err.log_context(Some(entry.shift_id.as_str()));
                                return Err(err);
                            }
                        }
                    }
                    Ok(None) => {}
                    Err(err) => {
                        err.log_context(None);
                        return Err(err);
                    }
                }
            }
            if !made_progress {
                break;
            }
        }
        let (processed, observed, dropped, lag_alert_active) = self.metrics().snapshot_counts();
        let latency_ms = self.metrics().snapshot_latency_ms();
        let status = self.capture_minimal_status()?;
        Ok(SubscriberRunSummary {
            processed,
            observed,
            dropped_l_step_below_threshold: dropped,
            lag_alert_active,
            latency_ms,
            outcomes,
            status,
        })
    }
    pub async fn run_until_shutdown(
        &self,
        mut shutdown: watch::Receiver<bool>,
    ) -> Result<SubscriberRunSummary> {
        self.metrics().mark_task_alive_now();
        let result = self.run_until_shutdown_inner(&mut shutdown).await;
        self.metrics().clear_task_alive();
        result
    }
    async fn run_until_shutdown_inner(
        &self,
        shutdown: &mut watch::Receiver<bool>,
    ) -> Result<SubscriberRunSummary> {
        let mut outcomes = Vec::new();
        let mut lag_exceeded_since = None;
        let poll = Duration::from_millis(self.config.tail_poll_interval_ms);
        loop {
            if *shutdown.borrow() {
                break;
            }
            let mut tails = self.open_tails()?;
            let mut made_progress = false;
            for tail in &mut tails {
                self.update_lag_alert(tail, &mut lag_exceeded_since)?;
                match tail.poll_next_line().await {
                    Ok(Some(entry)) => {
                        let outcome = self.process_shift_supervised(&entry).await;
                        match outcome {
                            Ok(outcome) => {
                                outcomes.push(outcome);
                                made_progress = true;
                            }
                            Err(err) => {
                                err.log_context(Some(entry.shift_id.as_str()));
                                return Err(err);
                            }
                        }
                    }
                    Ok(None) => {}
                    Err(err) => {
                        err.log_context(None);
                        return Err(err);
                    }
                }
            }
            if !made_progress {
                tokio::select! {
                    changed = shutdown.changed() => {
                        if changed.is_ok() && *shutdown.borrow() {
                            break;
                        }
                    }
                    _ = tokio::time::sleep(poll) => {}
                }
            }
        }
        let (processed, observed, dropped, lag_alert_active) = self.metrics().snapshot_counts();
        let latency_ms = self.metrics().snapshot_latency_ms();
        let status = self.capture_minimal_status()?;
        Ok(SubscriberRunSummary {
            processed,
            observed,
            dropped_l_step_below_threshold: dropped,
            lag_alert_active,
            latency_ms,
            outcomes,
            status,
        })
    }
    async fn process_shift_supervised(&self, entry: &ShiftEntry) -> Result<ShiftOutcome> {
        match AssertUnwindSafe(self.process_shift(entry, false))
            .catch_unwind()
            .await
        {
            Ok(result) => result,
            Err(payload) => {
                let detail = panic_payload_message(payload.as_ref());
                self.metrics().record_panic(detail.clone());
                error!(
                    error_code = "MEJEPA_SHIFT_SUBSCRIBER_PROCESS_PANIC",
                    shift_id = %entry.shift_id.0,
                    byte_offset = entry.byte_offset,
                    detail = %detail,
                    "ME-JEPA shift subscriber process_shift panic caught by supervisor"
                );
                Err(SubscriberError::ProcessPanic {
                    shift_id: entry.shift_id.0.clone(),
                    detail,
                })
            }
        }
    }
    fn open_tails(&self) -> Result<Vec<ShiftLogTail>> {
        let dir = &self.config.shift_log_dir;
        if !dir.is_dir() {
            return Err(SubscriberError::invalid(
                "shift_log_dir",
                format!("{} is not a directory", dir.display()),
            ));
        }
        let watermarks = self.watermark_writer.read_all()?;
        let mut tails = Vec::new();
        for entry in
            std::fs::read_dir(dir).map_err(|source| SubscriberError::io("read_dir", dir, source))?
        {
            let entry =
                entry.map_err(|source| SubscriberError::io("read_dir_entry", dir, source))?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("jsonl") {
                continue;
            }
            let session_hex = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .ok_or_else(|| {
                    SubscriberError::invalid(
                        "shift_log.file_name",
                        format!("{} has no UTF-8 file stem", path.display()),
                    )
                })?;
            decode_session_hex32(session_hex)?;
            let offset = watermarks
                .get(session_hex)
                .map(|record| record.last_consumed_byte_offset)
                .unwrap_or(0);
            tails.push(ShiftLogTail::new(path, offset));
        }
        Ok(tails)
    }
    fn update_lag_alert(
        &self,
        tail: &ShiftLogTail,
        exceeded_since: &mut Option<Instant>,
    ) -> Result<()> {
        let pending = count_pending_lines(tail.path(), tail.offset())?;
        if pending > self.config.lag_alert_threshold_shifts {
            let since = exceeded_since.get_or_insert_with(Instant::now);
            if since.elapsed() >= Duration::from_secs(self.config.lag_alert_sustain_seconds) {
                self.metrics().set_lag_alert_active(true);
                error!(
                    code = "MEJEPA_SHIFT_SUBSCRIBER_LAG_ALERT",
                    pending_shifts = pending,
                    threshold = self.config.lag_alert_threshold_shifts,
                    "ME-JEPA shift subscriber lag exceeded sustained threshold"
                );
            }
        } else {
            *exceeded_since = None;
            self.metrics().set_lag_alert_active(false);
        }
        Ok(())
    }
}
fn count_pending_lines(path: &Path, offset: u64) -> Result<usize> {
    let bytes = std::fs::read(path).map_err(|source| SubscriberError::io("read", path, source))?;
    let start = usize::try_from(offset).map_err(|err| {
        SubscriberError::invalid(
            "tail.offset",
            format!(
                "byte offset does not fit usize for {}: {err}",
                path.display()
            ),
        )
    })?;
    if start > bytes.len() {
        return Ok(0);
    }
    Ok(bytes[start..]
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .count())
}
fn panic_payload_message(payload: &(dyn Any + Send)) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_string()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "non-string panic payload".to_string()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MeJepaShiftSubscriberConfig, ShiftSubscriber};
    use context_graph_mejepa::{
        open_infer_rocksdb, CalibrationExample, CalibrationStore, EmbedderId, MejepaStore,
        RocksDbInferStore, TrainCertSummary,
    };
    use context_graph_mejepa_train::dda::count_persisted_dda_signals;
    use serde_json::json;
    use std::collections::BTreeMap;
    use std::path::PathBuf;
    #[tokio::test(flavor = "current_thread")]
    async fn run_until_idle_consumes_jsonl_and_persists_readback_state() {
        let temp = tempfile::tempdir().unwrap();
        let infer_db = temp.path().join("infer-db");
        let panel_db = temp.path().join("panel-db");
        let repo = temp.path().join("repo");
        let log_dir = temp.path().join("cgreality-shift-log");
        std::fs::create_dir_all(&repo).unwrap();
        std::fs::create_dir_all(&log_dir).unwrap();
        seed_infer_db(&infer_db).unwrap();

        let session = "11111111111111111111111111111111";
        let shift_id = "01J0123456789ABCDEF0123";
        let before = "def answer():\n    return 3\n";
        let after = "def answer():\n    return 4\n";
        let line = serde_json::to_string(&json!({
            "shift_id": shift_id,
            "timestamp_unix_ns": 1_772_000_000_000_000_000u128,
            "tool_name": "harness_apply_line_window_edit",
            "tool_use_id": "toolu-runner-test",
            "session_id": session,
            "subject": {
                "task_id": "runner_fsv_attempt",
                "path": "answer.py",
                "language": "python",
                "tests": ["test_answer"],
                "problem_statement": "answer returns four",
                "os": std::env::consts::OS
            },
            "before": {
                "text": before,
                "sha256": format!("sha256:{}", hex::encode(context_graph_mejepa::sha256_bytes(before.as_bytes())))
            },
            "after": {
                "text": after,
                "sha256": format!("sha256:{}", hex::encode(context_graph_mejepa::sha256_bytes(after.as_bytes())))
            },
            "delta_summary": {"commit_message": "phase7 runner test"},
            "verification": {
                "witness_chain_segment_hex": hex::encode(context_graph_mejepa::valid_witness_segment()),
                "l_step": 0.8,
                "delta_p": 0.9,
                "delta_k": 0.8,
                "delta_omega": 0.7,
                "delta_xi": 0.6,
                "actual_test_pass": [1.0]
            },
            "harness_transition_path": null
        }))
        .unwrap();
        std::fs::write(
            log_dir.join(format!("{session}.jsonl")),
            format!("{line}\n"),
        )
        .unwrap();

        let subscriber = ShiftSubscriber::open(MeJepaShiftSubscriberConfig {
            infer_db_path: infer_db.clone(),
            panel_db_path: panel_db,
            repo_root: repo,
            shift_log_dir: log_dir,
            l_step_observe_threshold: 0.05,
            max_concurrent_shifts: 1,
            lag_alert_threshold_shifts: 500,
            lag_alert_sustain_seconds: 60,
            tail_poll_interval_ms: 10,
        })
        .unwrap();
        let summary = subscriber.run_until_idle().await.unwrap();
        println!(
            "subscriber runner source-of-truth readback: {}",
            serde_json::to_string_pretty(&summary).unwrap()
        );
        assert_eq!(summary.processed, 1);
        assert_eq!(summary.observed, 1);
        assert!(matches!(
            summary.outcomes.as_slice(),
            [ShiftOutcome::Predicted {
                dda_signal_count: 1,
                ..
            }]
        ));

        let watermark = subscriber.watermark_writer.read_all().unwrap();
        assert_eq!(
            watermark[session].last_consumed_shift_id,
            shift_id.to_string()
        );
        assert_eq!(
            watermark[session].last_consumed_byte_offset,
            line.len() as u64 + 1
        );
        let cache_status = &summary.status["embedder_cache"];
        assert_eq!(cache_status["telemetry_present"], true);
        assert_eq!(cache_status["telemetry"]["misses"], 4);
        assert_eq!(cache_status["telemetry"]["writes"], 4);
        assert_eq!(cache_status["telemetry"]["entry_count"], 4);
        let telemetry_path = PathBuf::from(cache_status["telemetry_path"].as_str().unwrap());
        assert!(telemetry_path.is_file());
        let telemetry_readback: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&telemetry_path).unwrap()).unwrap();
        assert_eq!(telemetry_readback["misses"], 4);
        assert_eq!(telemetry_readback["writes"], 4);
        assert_eq!(telemetry_readback["entry_count"], 4);
        drop(subscriber);

        let db = open_infer_rocksdb(&infer_db).unwrap();
        let store = RocksDbInferStore::new(db.clone());
        let predictions = store
            .read_live_predictions(crate::models::decode_session_hex32(session).unwrap(), 10)
            .unwrap();
        assert_eq!(predictions.len(), 1);
        assert_eq!(count_persisted_dda_signals(db.as_ref()).unwrap(), 1);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn run_until_idle_skips_non_code_shift_and_advances_watermark() {
        let temp = tempfile::tempdir().unwrap();
        let infer_db = temp.path().join("infer-db");
        let panel_db = temp.path().join("panel-db");
        let repo = temp.path().join("repo");
        let log_dir = temp.path().join("cgreality-shift-log");
        std::fs::create_dir_all(&repo).unwrap();
        std::fs::create_dir_all(&log_dir).unwrap();
        seed_infer_db(&infer_db).unwrap();

        let session = "12121212121212121212121212121212";
        let shift_id = "01J00000000000000000001";
        let line = serde_json::to_string(&json!({
            "shift_id": shift_id,
            "timestamp_unix_ns": 1_772_000_000_000_000_000u128,
            "tool_name": "optimizer_record_decision",
            "tool_use_id": null,
            "session_id": session,
            "subject": {"type": "decision", "policy": "synthetic"},
            "before": {},
            "after": {},
            "delta_summary": {"summary": "non-code shift should only advance watermark"},
            "verification": {},
            "harness_transition_path": null
        }))
        .unwrap();
        std::fs::write(
            log_dir.join(format!("{session}.jsonl")),
            format!("{line}\n"),
        )
        .unwrap();

        let subscriber = ShiftSubscriber::open(MeJepaShiftSubscriberConfig {
            infer_db_path: infer_db.clone(),
            panel_db_path: panel_db,
            repo_root: repo,
            shift_log_dir: log_dir,
            l_step_observe_threshold: 0.05,
            max_concurrent_shifts: 1,
            lag_alert_threshold_shifts: 500,
            lag_alert_sustain_seconds: 60,
            tail_poll_interval_ms: 10,
        })
        .unwrap();
        let summary = subscriber.run_until_idle().await.unwrap();
        println!(
            "subscriber non-code skip source-of-truth readback: {}",
            serde_json::to_string_pretty(&summary).unwrap()
        );
        assert_eq!(summary.processed, 1);
        assert_eq!(summary.observed, 0);
        assert!(matches!(
            summary.outcomes.as_slice(),
            [ShiftOutcome::Skipped {
                reason: crate::models::SkipReason::NoOracleSignal,
                ..
            }]
        ));

        let watermark = subscriber.watermark_writer.read_all().unwrap();
        assert_eq!(
            watermark[session].last_consumed_shift_id,
            shift_id.to_string()
        );
        assert_eq!(
            watermark[session].last_consumed_byte_offset,
            line.len() as u64 + 1
        );
        drop(subscriber);

        let db = open_infer_rocksdb(&infer_db).unwrap();
        let store = RocksDbInferStore::new(db.clone());
        let predictions = store
            .read_live_predictions(crate::models::decode_session_hex32(session).unwrap(), 10)
            .unwrap();
        assert!(predictions.is_empty());
        assert_eq!(count_persisted_dda_signals(db.as_ref()).unwrap(), 0);
    }

    fn seed_infer_db(path: &Path) -> Result<()> {
        let db = open_infer_rocksdb(path)?;
        let calibration = CalibrationStore::new(db.clone(), 30)?;
        let examples = (0..40)
            .map(|idx| CalibrationExample {
                language: context_graph_mejepa::Language::Python,
                predicted_test_pass: vec![if idx % 10 == 0 { 0.2 } else { 0.95 }],
                actual_test_pass: vec![if idx % 10 == 0 { 0.0 } else { 1.0 }],
            })
            .collect::<Vec<_>>();
        let norms = vec![0.01; examples.len()];
        calibration.calibrate(
            &examples,
            &norms,
            0.10,
            30,
            0.30,
            [7; 32],
            BTreeMap::<EmbedderId, String>::new(),
        )?;
        let cf = db
            .cf_handle(context_graph_mejepa_cf::CF_MEJEPA_TRAIN_CERTS)
            .ok_or_else(|| SubscriberError::invalid("cf", "missing train cert CF"))?;
        let cert = TrainCertSummary {
            step: 1,
            delta_omega: 0.8,
            delta_xi: 0.8,
            witness_offset: 44,
            // #699: test-fixture cert; bump to 1 so the downstream verify
            // flow exercises the Measured arm.
            predictor_parameter_update_count: 1,
        };
        db.put_cf(cf, b"cert:runner:0001", bincode::serialize(&cert)?)?;
        Ok(())
    }
}
