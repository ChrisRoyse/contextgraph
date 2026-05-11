// Inspired by ruvnet/RuVector at HEAD ef5274c2 (clean-room reimplementation).

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use context_graph_mejepa::{
    build_deterministic_compiler, materialize_inference_panels, open_infer_rocksdb,
    CalibrationExample, CalibrationStore, MeJepaInferConfig, MejepaStore, PatchBundle,
    RealityPrediction, RocksDbInferStore, TaskContext, TrainCertSummary,
};
use context_graph_mejepa_instruments::materialize::TimeStep;
use context_graph_mejepa_instruments::panel_json::{PanelEnvelope, PanelProvenance};
use context_graph_mejepa_instruments::panel_store::{PanelKey, PanelStore};
use rocksdb::WriteOptions;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tracing::info;

use crate::models::{
    encode_session_hex, ShiftEntry, ShiftOutcome, SkipReason, SubscriberMetrics, UtmlFactorBundle,
    WatermarkRecord,
};
use crate::shift_transform::{
    required_f32_array, shift_to_inference, timestamp_ns_to_ms, utml_bundle,
};
use crate::{Result, SubscriberError, WatermarkWriter};

#[derive(Debug, Clone)]
pub struct MeJepaShiftSubscriberConfig {
    pub infer_db_path: PathBuf,
    pub panel_db_path: PathBuf,
    pub repo_root: PathBuf,
    pub shift_log_dir: PathBuf,
    pub l_step_observe_threshold: f32,
    pub max_concurrent_shifts: usize,
    pub lag_alert_threshold_shifts: usize,
    pub lag_alert_sustain_seconds: u64,
    pub tail_poll_interval_ms: u64,
}

impl MeJepaShiftSubscriberConfig {
    pub fn validate(&self) -> Result<()> {
        if self.infer_db_path.as_os_str().is_empty() {
            return Err(SubscriberError::invalid(
                "infer_db_path",
                "path must be non-empty",
            ));
        }
        if self.panel_db_path.as_os_str().is_empty() {
            return Err(SubscriberError::invalid(
                "panel_db_path",
                "path must be non-empty",
            ));
        }
        if self.repo_root.as_os_str().is_empty() {
            return Err(SubscriberError::invalid(
                "repo_root",
                "path must be non-empty",
            ));
        }
        if self.shift_log_dir.as_os_str().is_empty() {
            return Err(SubscriberError::invalid(
                "shift_log_dir",
                "path must be non-empty",
            ));
        }
        if !self.l_step_observe_threshold.is_finite()
            || !(0.01..=0.20).contains(&self.l_step_observe_threshold)
        {
            return Err(SubscriberError::invalid(
                "l_step_observe_threshold",
                "threshold must be finite and in [0.01, 0.20]",
            ));
        }
        if self.max_concurrent_shifts != 1 {
            return Err(SubscriberError::invalid(
                "max_concurrent_shifts",
                "v1.0 requires max_concurrent_shifts == 1",
            ));
        }
        if !(100..=5_000).contains(&self.lag_alert_threshold_shifts) {
            return Err(SubscriberError::invalid(
                "lag_alert_threshold_shifts",
                "threshold must be in [100, 5000]",
            ));
        }
        if !(10..=300).contains(&self.lag_alert_sustain_seconds) {
            return Err(SubscriberError::invalid(
                "lag_alert_sustain_seconds",
                "sustain window must be in [10, 300] seconds",
            ));
        }
        if !(10..=5_000).contains(&self.tail_poll_interval_ms) {
            return Err(SubscriberError::invalid(
                "tail_poll_interval_ms",
                "poll interval must be in [10, 5000] ms",
            ));
        }
        Ok(())
    }
}

pub struct ShiftSubscriber {
    pub(crate) config: MeJepaShiftSubscriberConfig,
    db: Arc<rocksdb::DB>,
    panel_store: PanelStore,
    store: RocksDbInferStore,
    pub(crate) watermark_writer: WatermarkWriter,
    metrics: Arc<SubscriberMetrics>,
}

impl ShiftSubscriber {
    pub fn open(config: MeJepaShiftSubscriberConfig) -> Result<Self> {
        config.validate()?;
        let db = open_infer_rocksdb(&config.infer_db_path)?;
        let panel_store = PanelStore::open(&config.panel_db_path)?;
        let store = RocksDbInferStore::new(db.clone());
        let watermark_writer = WatermarkWriter::new(db.clone())?;
        Ok(Self {
            config,
            db,
            panel_store,
            store,
            watermark_writer,
            metrics: Arc::new(SubscriberMetrics::default()),
        })
    }

    pub fn metrics(&self) -> Arc<SubscriberMetrics> {
        self.metrics.clone()
    }

    pub fn read_watermarks(&self) -> Result<BTreeMap<String, WatermarkRecord>> {
        self.watermark_writer.read_all()
    }

    pub fn process_shift(&self, entry: &ShiftEntry, replay: bool) -> Result<ShiftOutcome> {
        let started = Instant::now();
        if let Some(existing) = self.watermark_writer.read(entry.session_id)? {
            if existing.source_log_path.as_deref()
                == Some(&entry.source_log_path.display().to_string())
                && existing.last_consumed_byte_offset >= entry.next_byte_offset
            {
                return Ok(ShiftOutcome::Skipped {
                    reason: SkipReason::AlreadyConsumed,
                    watermark_key: Some(WatermarkWriter::key_for_session_hex(&existing.session_id)),
                    watermark_offset: Some(existing.last_consumed_byte_offset),
                });
            }
        }
        let (patch, context, source_sha, attempt_id) =
            shift_to_inference(entry, self.config.repo_root.clone())?;
        let compiler = self.compiler()?;
        let mut prediction = compiler.compile(&patch, &context)?;
        prediction.created_at_unix_ms = timestamp_ns_to_ms(entry.timestamp_unix_ns)?;
        let prediction = RealityPrediction::try_new(prediction)?;
        self.store.write_live_prediction(&prediction)?;
        let readback = MejepaStore::read_live_predictions(&self.store, context.session_id, 1000)?;
        if !readback.iter().any(|row| {
            row.prediction_id == prediction.prediction_id
                && row.created_at_unix_ms == prediction.created_at_unix_ms
        }) {
            return Err(SubscriberError::ProcessFailed {
                shift_id: entry.shift_id.0.clone(),
                detail: "live prediction readback did not contain the written row".to_string(),
            });
        }
        self.persist_panels(&patch, &context, &attempt_id, source_sha)?;
        let observed = self.persist_oracle_example(entry, &prediction, replay)?;
        let record = WatermarkRecord {
            session_id: encode_session_hex(entry.session_id),
            last_consumed_shift_id: entry.shift_id.0.clone(),
            last_consumed_byte_offset: entry.next_byte_offset,
            last_advanced_at_unix_seconds: chrono_like_now_seconds(),
            producer_tool_name: Some(if replay {
                "mcp__cgreality__mejepa_observe_shift".to_string()
            } else {
                entry.tool_name.clone()
            }),
            source_log_path: Some(entry.source_log_path.display().to_string()),
        };
        let watermark_key = self.watermark_writer.write_watermark(&record)?;
        self.metrics.mark_processed();
        if observed {
            self.metrics.mark_observed();
        }
        let latency_ms = started.elapsed().as_millis().try_into().unwrap_or(u64::MAX);
        self.metrics.record_latency_ms(latency_ms);
        info!(
            error_code = "MEJEPA_SHIFT_SUBSCRIBER_PROCESSED",
            shift_id = %entry.shift_id.0,
            session_id = %record.session_id,
            prediction_id = %hex::encode(prediction.prediction_id),
            latency_ms,
            "processed ME-JEPA shift"
        );
        Ok(ShiftOutcome::Predicted {
            prediction_id: hex::encode(prediction.prediction_id),
            observed,
            watermark_key,
            watermark_offset: record.last_consumed_byte_offset,
            latency_ms,
        })
    }

    pub fn capture_minimal_status(&self) -> Result<Value> {
        let (processed, observed, dropped, lag_alert_active) = self.metrics.snapshot_counts();
        let watermarks = self.watermark_writer.read_all()?;
        let last_watermark_per_session = watermarks
            .iter()
            .map(|(session, record)| (session.clone(), record.last_consumed_shift_id.clone()))
            .collect::<BTreeMap<_, _>>();
        Ok(json!({
            "subscriber_running": true,
            "task_alive_since": Value::Null,
            "processed_count": processed,
            "observed_count": observed,
            "dropped_l_step_below_threshold_count": dropped,
            "lag_alert_active": lag_alert_active,
            "latency_ms": self.metrics.snapshot_latency_ms(),
            "last_panic": Value::Null,
            "last_watermark_per_session": last_watermark_per_session
        }))
    }

    fn compiler(&self) -> Result<context_graph_mejepa::MeJepaCompiler> {
        let calibration = CalibrationStore::new(self.db.clone(), 30)?;
        let store = Arc::new(self.store.clone());
        Ok(build_deterministic_compiler(
            self.config.repo_root.clone(),
            store,
            calibration,
            MeJepaInferConfig::default(),
        )?)
    }

    fn persist_panels(
        &self,
        patch: &PatchBundle,
        context: &TaskContext,
        attempt_id: &str,
        source_sha: [u8; 32],
    ) -> Result<()> {
        let (t0, t1, t2) = materialize_inference_panels(patch, context)?;
        let provenance = PanelProvenance {
            code_version: env!("CARGO_PKG_VERSION").to_string(),
            embedder_versions: BTreeMap::from([(
                "deterministic_inference_panel".to_string(),
                env!("CARGO_PKG_VERSION").to_string(),
            )]),
            corpus_sha: hex::encode(patch.patch_sha),
            frozen_at_unix_ms: chrono_like_now_seconds() * 1000,
            source_sha256: hex::encode(source_sha),
        };
        for (step, panel) in [(TimeStep::T0, t0), (TimeStep::T1, t1), (TimeStep::T2, t2)] {
            let key = PanelKey::new(attempt_id, step)?;
            let envelope = PanelEnvelope::try_new(step, panel, provenance.clone())?;
            self.panel_store.put_envelope(&key, &envelope)?;
            let loaded = self.panel_store.get_envelope(&key)?.ok_or_else(|| {
                SubscriberError::invalid("panel.readback", "panel row absent after write")
            })?;
            if loaded.panel_hash != envelope.panel_hash {
                return Err(SubscriberError::invalid(
                    "panel.readback",
                    "panel hash readback mismatch",
                ));
            }
        }
        Ok(())
    }

    fn persist_oracle_example(
        &self,
        entry: &ShiftEntry,
        prediction: &context_graph_mejepa::RealityPrediction,
        replay: bool,
    ) -> Result<bool> {
        let Some(bundle) = utml_bundle(&entry.verification)? else {
            return Ok(false);
        };
        let actual = required_f32_array(&entry.verification, "actual_test_pass")?;
        let example = CalibrationExample {
            language: prediction.language,
            predicted_test_pass: prediction.predicted_test_pass.clone(),
            actual_test_pass: actual,
        };
        let cf = self
            .db
            .cf_handle(context_graph_mejepa_cf::CF_MEJEPA_ORACLE_VERDICTS)
            .ok_or_else(|| {
                SubscriberError::invalid(
                    "rocksdb.column_family",
                    "missing CF_MEJEPA_ORACLE_VERDICTS",
                )
            })?;
        let mut hasher = Sha256::new();
        hasher.update(entry.shift_id.0.as_bytes());
        hasher.update(prediction.prediction_id);
        let key = hasher.finalize();
        let value = bincode::serialize(&example)?;
        let mut opts = WriteOptions::default();
        opts.set_sync(true);
        self.db.put_cf_opt(cf, &key, &value, &opts)?;
        let readback = self.db.get_cf(cf, &key)?.ok_or_else(|| {
            SubscriberError::invalid("oracle_verdict.readback", "oracle verdict row absent")
        })?;
        if readback != value {
            return Err(SubscriberError::invalid(
                "oracle_verdict.readback",
                "oracle verdict readback mismatch",
            ));
        }
        self.persist_train_cert(entry, &bundle, replay)?;
        if bundle.l_step < self.config.l_step_observe_threshold {
            self.metrics.mark_l_step_dropped();
            tracing::info!(
                error_code = "MEJEPA_SHIFT_OBSERVE_DROPPED",
                shift_id = %entry.shift_id.0,
                l_step = bundle.l_step,
                threshold = self.config.l_step_observe_threshold,
                "oracle verdict persisted but online observe skipped below L_step threshold"
            );
            return Ok(false);
        }
        Ok(true)
    }

    fn persist_train_cert(
        &self,
        entry: &ShiftEntry,
        bundle: &UtmlFactorBundle,
        replay: bool,
    ) -> Result<()> {
        let cf = self
            .db
            .cf_handle(context_graph_mejepa_cf::CF_MEJEPA_TRAIN_CERTS)
            .ok_or_else(|| {
                SubscriberError::invalid("rocksdb.column_family", "missing CF_MEJEPA_TRAIN_CERTS")
            })?;
        let cert = TrainCertSummary {
            step: timestamp_ns_to_ms(entry.timestamp_unix_ns)? as u64,
            delta_omega: bundle.delta_omega,
            delta_xi: bundle.delta_xi,
            witness_offset: entry.byte_offset,
        };
        cert.validate()?;
        let key = format!(
            "cert:phase7:{}:{}",
            entry.shift_id.0,
            if replay { "replay" } else { "live" }
        );
        let value = bincode::serialize(&cert)?;
        let mut opts = WriteOptions::default();
        opts.set_sync(true);
        self.db.put_cf_opt(cf, key.as_bytes(), &value, &opts)?;
        let readback = self.db.get_cf(cf, key.as_bytes())?.ok_or_else(|| {
            SubscriberError::invalid("train_cert.readback", "train cert row absent")
        })?;
        if readback != value {
            return Err(SubscriberError::invalid(
                "train_cert.readback",
                "train cert readback mismatch",
            ));
        }
        Ok(())
    }
}

fn chrono_like_now_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time before unix epoch")
        .as_secs() as i64
}
