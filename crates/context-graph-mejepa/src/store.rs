use std::sync::Arc;

use context_graph_mejepa_cf::{
    CF_MEJEPA_CONSTELLATION_PATTERNS, CF_MEJEPA_CONTRADICTION_THRESHOLDS,
    CF_MEJEPA_FAILURE_FINGERPRINTS, CF_MEJEPA_HIERARCHICAL_PREDICTIONS,
};
use rocksdb::{Direction, IteratorMode, WriteOptions, DB};

use crate::calibration::{
    cf, CF_MEJEPA_LIVE_PREDICTIONS, CF_MEJEPA_ORACLE_VERDICTS, CF_MEJEPA_TRAIN_CERTS,
};
use crate::compiler::{ColdCellMetric, MejepaStore, TrainCertSummary};
use crate::conformal::CalibrationExample;
use crate::constellation_intelligence::{
    constellation_pattern_key, ConstellationRelationshipPattern,
};
use crate::contradiction::{contradiction_threshold_key, ContradictionThresholds};
use crate::error::MejepaInferError;
use crate::failure_fingerprint::FailureShapeFingerprint;
use crate::ood_harvest::OodCalibrationReport;
use crate::park_list::{
    clear_park_list_entry as clear_park_list_entry_row,
    read_park_list_entry as read_park_list_entry_row,
    record_park_list_failure as record_park_list_failure_row, ParkListEntry,
};
use crate::system_cost::SystemCostCounters;
use crate::types::{
    decode_hierarchical_prediction_record, decode_reality_prediction, ChunkId, DdaSignals,
    HierarchicalPredictionRecord, PanelId, RealityPrediction,
};

#[derive(Clone)]
pub struct RocksDbInferStore {
    db: Arc<DB>,
    system_cost_counters: Option<Arc<SystemCostCounters>>,
}

impl RocksDbInferStore {
    pub fn new(db: Arc<DB>) -> Self {
        Self {
            db,
            system_cost_counters: None,
        }
    }

    pub fn new_with_system_cost_counters(
        db: Arc<DB>,
        system_cost_counters: Arc<SystemCostCounters>,
    ) -> Self {
        Self {
            db,
            system_cost_counters: Some(system_cost_counters),
        }
    }

    pub fn db(&self) -> Arc<DB> {
        self.db.clone()
    }

    pub fn system_cost_counters(&self) -> Option<Arc<SystemCostCounters>> {
        self.system_cost_counters.clone()
    }

    fn live_key(prediction: &RealityPrediction) -> Vec<u8> {
        let mut key = Vec::with_capacity(40);
        key.extend_from_slice(&prediction.session_id);
        key.extend_from_slice(&prediction.created_at_unix_ms.to_be_bytes());
        key.extend_from_slice(&prediction.prediction_id);
        key
    }

    fn hierarchical_key(record: &HierarchicalPredictionRecord) -> Vec<u8> {
        let mut key = Vec::with_capacity(40);
        key.extend_from_slice(&record.session_id);
        key.extend_from_slice(&record.created_at_unix_ms.to_be_bytes());
        key.extend_from_slice(&record.prediction_id);
        key
    }

    fn cold_cell_metric_key(metric: &ColdCellMetric) -> Result<Vec<u8>, MejepaInferError> {
        metric.validate()?;
        if metric.cell_id.as_bytes().contains(&0) {
            return Err(MejepaInferError::InvalidInput {
                field: "cold_cell_metric.cell_id".to_string(),
                detail: "cell_id must not contain NUL bytes".to_string(),
            });
        }
        let mut key = Vec::with_capacity(metric.cell_id.len() + 1 + 8 + 16);
        key.extend_from_slice(metric.cell_id.as_bytes());
        key.push(0);
        key.extend_from_slice(&metric.created_at_unix_ms.to_be_bytes());
        key.extend_from_slice(&metric.prediction_id);
        Ok(key)
    }

    pub fn write_contradiction_thresholds(
        &self,
        thresholds: &ContradictionThresholds,
    ) -> Result<(), MejepaInferError> {
        thresholds.validate()?;
        let cf = cf(&self.db, CF_MEJEPA_CONTRADICTION_THRESHOLDS)?;
        let key = contradiction_threshold_key(&thresholds.cell_id)?;
        let value = bincode::serialize(thresholds)?;
        let mut opts = WriteOptions::default();
        opts.set_sync(true);
        self.db.put_cf_opt(cf, &key, &value, &opts)?;
        let readback = self
            .db
            .get_cf(cf, &key)?
            .ok_or_else(|| MejepaInferError::InvalidInput {
                field: "contradiction_thresholds".to_string(),
                detail: "read-after-write could not find threshold row".to_string(),
            })?;
        if readback != value {
            return Err(MejepaInferError::InvalidInput {
                field: "contradiction_thresholds".to_string(),
                detail: "read-after-write bytes differ from threshold payload".to_string(),
            });
        }
        let decoded: ContradictionThresholds = bincode::deserialize(&readback)?;
        decoded.validate()?;
        if decoded != *thresholds {
            return Err(MejepaInferError::InvalidInput {
                field: "contradiction_thresholds".to_string(),
                detail: "read-after-write decoded threshold does not match input".to_string(),
            });
        }
        if let Some(counters) = &self.system_cost_counters {
            counters.record_rocksdb_write(value.len() as u64);
        }
        Ok(())
    }

    pub fn write_constellation_relationship_pattern(
        &self,
        pattern: &ConstellationRelationshipPattern,
    ) -> Result<(), MejepaInferError> {
        pattern.validate()?;
        let cf = cf(&self.db, CF_MEJEPA_CONSTELLATION_PATTERNS)?;
        let key = constellation_pattern_key(&pattern.pattern_id)?;
        let value = bincode::serialize(pattern)?;
        let mut opts = WriteOptions::default();
        opts.set_sync(true);
        self.db.put_cf_opt(cf, &key, &value, &opts)?;
        let readback = self
            .db
            .get_cf(cf, &key)?
            .ok_or_else(|| MejepaInferError::InvalidInput {
                field: "constellation_pattern".to_string(),
                detail: "read-after-write could not find pattern row".to_string(),
            })?;
        if readback != value {
            return Err(MejepaInferError::InvalidInput {
                field: "constellation_pattern".to_string(),
                detail: "read-after-write bytes differ from pattern payload".to_string(),
            });
        }
        let decoded: ConstellationRelationshipPattern = bincode::deserialize(&readback)?;
        decoded.validate()?;
        if decoded != *pattern {
            return Err(MejepaInferError::InvalidInput {
                field: "constellation_pattern".to_string(),
                detail: "read-after-write decoded pattern does not match input".to_string(),
            });
        }
        if let Some(counters) = &self.system_cost_counters {
            counters.record_rocksdb_write(value.len() as u64);
        }
        Ok(())
    }

    pub fn read_constellation_relationship_patterns(
        &self,
        limit: usize,
    ) -> Result<Vec<ConstellationRelationshipPattern>, MejepaInferError> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        let cf = cf(&self.db, CF_MEJEPA_CONSTELLATION_PATTERNS)?;
        let mut out = Vec::with_capacity(limit);
        for item in self.db.iterator_cf(cf, IteratorMode::Start) {
            let (_key, value) = item?;
            let pattern: ConstellationRelationshipPattern = bincode::deserialize(&value)?;
            pattern.validate()?;
            out.push(pattern);
            if out.len() == limit {
                break;
            }
        }
        Ok(out)
    }
}

impl MejepaStore for RocksDbInferStore {
    fn read_recent_train_certs(
        &self,
        limit: usize,
    ) -> Result<Vec<TrainCertSummary>, MejepaInferError> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        let cf = cf(&self.db, CF_MEJEPA_TRAIN_CERTS)?;
        let mut out = Vec::with_capacity(limit);
        for item in self.db.iterator_cf(cf, IteratorMode::End) {
            let (_key, value) = item?;
            let cert: TrainCertSummary = bincode::deserialize(&value)?;
            cert.validate()?;
            out.push(cert);
            if out.len() == limit {
                break;
            }
        }
        Ok(out)
    }

    fn write_live_prediction(
        &self,
        prediction: &RealityPrediction,
    ) -> Result<(), MejepaInferError> {
        prediction.validate()?;
        let cf = cf(&self.db, CF_MEJEPA_LIVE_PREDICTIONS)?;
        let key = Self::live_key(prediction);
        let value = bincode::serialize(prediction)?;
        let mut opts = WriteOptions::default();
        opts.set_sync(true);
        self.db.put_cf_opt(cf, &key, &value, &opts)?;
        let readback = self
            .db
            .get_cf(cf, &key)?
            .ok_or_else(|| MejepaInferError::InvalidInput {
                field: "live_predictions".to_string(),
                detail: "read-after-write could not find persisted prediction row".to_string(),
            })?;
        if readback != value {
            return Err(MejepaInferError::InvalidInput {
                field: "live_predictions".to_string(),
                detail: "read-after-write bytes differ from persisted prediction payload"
                    .to_string(),
            });
        }
        let decoded = decode_reality_prediction(&readback)?;
        if decoded != *prediction {
            return Err(MejepaInferError::InvalidInput {
                field: "live_predictions".to_string(),
                detail: "read-after-write decoded prediction does not match input".to_string(),
            });
        }
        if let Some(counters) = &self.system_cost_counters {
            counters.record_rocksdb_write(value.len() as u64);
        }
        Ok(())
    }

    fn read_live_predictions(
        &self,
        session_id: [u8; 16],
        limit: u32,
    ) -> Result<Vec<RealityPrediction>, MejepaInferError> {
        if !(1..=1000).contains(&limit) {
            return Err(MejepaInferError::InvalidInput {
                field: "limit".to_string(),
                detail: format!("limit must be in [1, 1000], got {limit}"),
            });
        }
        let limit = limit as usize;
        let cf = cf(&self.db, CF_MEJEPA_LIVE_PREDICTIONS)?;
        let mut end = Vec::with_capacity(24);
        end.extend_from_slice(&session_id);
        end.extend_from_slice(&i64::MAX.to_be_bytes());
        let iter = self
            .db
            .iterator_cf(cf, IteratorMode::From(&end, Direction::Reverse));
        let mut out = Vec::with_capacity(limit);
        for item in iter {
            let (key, value) = item?;
            if !key.starts_with(&session_id) {
                break;
            }
            if key.len() != 40 {
                return Err(MejepaInferError::DimMismatch {
                    expected: 40,
                    actual: key.len(),
                    context:
                        "live prediction key must be session_id || created_at || prediction_id"
                            .to_string(),
                });
            }
            let prediction = decode_reality_prediction(&value)?;
            if prediction.session_id != session_id {
                return Err(MejepaInferError::InvalidInput {
                    field: "live_predictions.session_id".to_string(),
                    detail: "payload session_id does not match key prefix".to_string(),
                });
            }
            let mut key_created_at = [0u8; 8];
            key_created_at.copy_from_slice(&key[16..24]);
            let created_at_unix_ms = i64::from_be_bytes(key_created_at);
            if prediction.created_at_unix_ms != created_at_unix_ms {
                return Err(MejepaInferError::InvalidInput {
                    field: "live_predictions.created_at_unix_ms".to_string(),
                    detail: "payload created_at_unix_ms does not match key timestamp".to_string(),
                });
            }
            let mut key_prediction_id = [0u8; 16];
            key_prediction_id.copy_from_slice(&key[24..40]);
            if prediction.prediction_id != key_prediction_id {
                return Err(MejepaInferError::InvalidInput {
                    field: "live_predictions.prediction_id".to_string(),
                    detail: "payload prediction_id does not match key suffix".to_string(),
                });
            }
            out.push(prediction);
            if out.len() == limit {
                break;
            }
        }
        Ok(out)
    }

    fn write_hierarchical_prediction(
        &self,
        record: &HierarchicalPredictionRecord,
    ) -> Result<(), MejepaInferError> {
        record.validate()?;
        let cf = cf(&self.db, CF_MEJEPA_HIERARCHICAL_PREDICTIONS)?;
        let key = Self::hierarchical_key(record);
        let value = bincode::serialize(record)?;
        let mut opts = WriteOptions::default();
        opts.set_sync(true);
        self.db.put_cf_opt(cf, &key, &value, &opts)?;
        let readback = self
            .db
            .get_cf(cf, &key)?
            .ok_or_else(|| MejepaInferError::InvalidInput {
                field: "hierarchical_predictions".to_string(),
                detail: "read-after-write could not find persisted hierarchy row".to_string(),
            })?;
        if readback != value {
            return Err(MejepaInferError::InvalidInput {
                field: "hierarchical_predictions".to_string(),
                detail: "read-after-write bytes differ from persisted hierarchy payload"
                    .to_string(),
            });
        }
        let decoded = decode_hierarchical_prediction_record(&readback)?;
        decoded.validate()?;
        if decoded != *record {
            return Err(MejepaInferError::InvalidInput {
                field: "hierarchical_predictions".to_string(),
                detail: "read-after-write decoded hierarchy does not match input".to_string(),
            });
        }
        if let Some(counters) = &self.system_cost_counters {
            counters.record_rocksdb_write(value.len() as u64);
        }
        Ok(())
    }

    fn read_hierarchical_predictions(
        &self,
        session_id: [u8; 16],
        limit: u32,
    ) -> Result<Vec<HierarchicalPredictionRecord>, MejepaInferError> {
        if !(1..=1000).contains(&limit) {
            return Err(MejepaInferError::InvalidInput {
                field: "limit".to_string(),
                detail: format!("limit must be in [1, 1000], got {limit}"),
            });
        }
        let limit = limit as usize;
        let cf = cf(&self.db, CF_MEJEPA_HIERARCHICAL_PREDICTIONS)?;
        let mut end = Vec::with_capacity(24);
        end.extend_from_slice(&session_id);
        end.extend_from_slice(&i64::MAX.to_be_bytes());
        let iter = self
            .db
            .iterator_cf(cf, IteratorMode::From(&end, Direction::Reverse));
        let mut out = Vec::with_capacity(limit);
        for item in iter {
            let (key, value) = item?;
            if !key.starts_with(&session_id) {
                break;
            }
            if key.len() != 40 {
                return Err(MejepaInferError::DimMismatch {
                    expected: 40,
                    actual: key.len(),
                    context:
                        "hierarchical prediction key must be session_id || created_at || prediction_id"
                            .to_string(),
                });
            }
            let record = decode_hierarchical_prediction_record(&value)?;
            if record.session_id != session_id {
                return Err(MejepaInferError::InvalidInput {
                    field: "hierarchical_predictions.session_id".to_string(),
                    detail: "payload session_id does not match key prefix".to_string(),
                });
            }
            let mut key_created_at = [0u8; 8];
            key_created_at.copy_from_slice(&key[16..24]);
            let created_at_unix_ms = i64::from_be_bytes(key_created_at);
            if record.created_at_unix_ms != created_at_unix_ms {
                return Err(MejepaInferError::InvalidInput {
                    field: "hierarchical_predictions.created_at_unix_ms".to_string(),
                    detail: "payload created_at_unix_ms does not match key timestamp".to_string(),
                });
            }
            let mut key_prediction_id = [0u8; 16];
            key_prediction_id.copy_from_slice(&key[24..40]);
            if record.prediction_id != key_prediction_id {
                return Err(MejepaInferError::InvalidInput {
                    field: "hierarchical_predictions.prediction_id".to_string(),
                    detail: "payload prediction_id does not match key suffix".to_string(),
                });
            }
            out.push(record);
            if out.len() == limit {
                break;
            }
        }
        Ok(out)
    }

    fn session_known(&self, session_id: [u8; 16]) -> Result<bool, MejepaInferError> {
        let cf = cf(&self.db, CF_MEJEPA_LIVE_PREDICTIONS)?;
        let mut end = Vec::with_capacity(24);
        end.extend_from_slice(&session_id);
        end.extend_from_slice(&i64::MAX.to_be_bytes());
        let mut iter = self
            .db
            .iterator_cf(cf, IteratorMode::From(&end, Direction::Reverse));
        let Some(item) = iter.next() else {
            return Ok(false);
        };
        let (key, value) = item?;
        if !key.starts_with(&session_id) {
            return Ok(false);
        }
        if key.len() != 40 {
            return Err(MejepaInferError::DimMismatch {
                expected: 40,
                actual: key.len(),
                context: "live prediction key must be session_id || created_at || prediction_id"
                    .to_string(),
            });
        }
        let prediction = decode_reality_prediction(&value)?;
        if prediction.session_id != session_id {
            return Err(MejepaInferError::InvalidInput {
                field: "live_predictions.session_id".to_string(),
                detail: "payload session_id does not match key prefix".to_string(),
            });
        }
        Ok(true)
    }

    fn read_recent_calibration_examples(
        &self,
        limit: usize,
    ) -> Result<Vec<CalibrationExample>, MejepaInferError> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        let cf = cf(&self.db, CF_MEJEPA_ORACLE_VERDICTS)?;
        let mut out = Vec::with_capacity(limit);
        for item in self.db.iterator_cf(cf, IteratorMode::End) {
            let (_key, value) = item?;
            let example: CalibrationExample = bincode::deserialize(&value)?;
            out.push(example);
            if out.len() == limit {
                break;
            }
        }
        Ok(out)
    }

    fn read_contradiction_thresholds(
        &self,
        cell_id: &str,
    ) -> Result<Option<ContradictionThresholds>, MejepaInferError> {
        let cf = cf(&self.db, CF_MEJEPA_CONTRADICTION_THRESHOLDS)?;
        let Some(bytes) = self.db.get_cf(cf, contradiction_threshold_key(cell_id)?)? else {
            return Ok(None);
        };
        let thresholds: ContradictionThresholds = bincode::deserialize(&bytes)?;
        thresholds.validate()?;
        Ok(Some(thresholds))
    }

    fn read_dda_signals(
        &self,
        panel_id: &PanelId,
        chunk_id: &ChunkId,
    ) -> Result<Option<DdaSignals>, MejepaInferError> {
        chunk_id.validate("dda_signal.chunk_id")?;
        let cf = cf(&self.db, context_graph_mejepa_cf::CF_MEJEPA_DDA_SIGNALS)?;
        let key = bincode::serialize(&(panel_id, chunk_id))?;
        let Some(bytes) = self.db.get_cf(cf, key)? else {
            return Ok(None);
        };
        let signals: DdaSignals = serde_json::from_slice(&bytes)?;
        signals.validate()?;
        Ok(Some(signals))
    }

    fn read_park_list_entry(
        &self,
        prediction_id: [u8; 16],
    ) -> Result<Option<ParkListEntry>, MejepaInferError> {
        read_park_list_entry_row(&self.db, prediction_id)
    }

    fn record_park_list_failure(
        &self,
        prediction_id: [u8; 16],
        now_unix_ms: i64,
        error_code: &str,
    ) -> Result<ParkListEntry, MejepaInferError> {
        let entry = record_park_list_failure_row(&self.db, prediction_id, now_unix_ms, error_code)?;
        if let Some(counters) = &self.system_cost_counters {
            counters.record_rocksdb_write(serde_json::to_vec(&entry)?.len() as u64);
        }
        Ok(entry)
    }

    fn clear_park_list_entry(&self, prediction_id: [u8; 16]) -> Result<(), MejepaInferError> {
        clear_park_list_entry_row(&self.db, prediction_id)
    }

    fn record_cold_cell_metric(&self, metric: &ColdCellMetric) -> Result<(), MejepaInferError> {
        metric.validate()?;
        let cf = cf(
            &self.db,
            context_graph_mejepa_cf::CF_MEJEPA_COLD_CELL_METRICS,
        )?;
        let key = Self::cold_cell_metric_key(metric)?;
        let value = bincode::serialize(metric)?;
        let mut opts = WriteOptions::default();
        opts.set_sync(true);
        self.db.put_cf_opt(cf, &key, &value, &opts)?;
        let readback = self
            .db
            .get_cf(cf, &key)?
            .ok_or_else(|| MejepaInferError::InvalidInput {
                field: "cold_cell_metrics".to_string(),
                detail: "read-after-write could not find persisted cold-cell metric row"
                    .to_string(),
            })?;
        if readback != value {
            return Err(MejepaInferError::InvalidInput {
                field: "cold_cell_metrics".to_string(),
                detail: "read-after-write bytes differ from persisted metric payload".to_string(),
            });
        }
        let decoded: ColdCellMetric = bincode::deserialize(&readback)?;
        decoded.validate()?;
        if decoded != *metric {
            return Err(MejepaInferError::InvalidInput {
                field: "cold_cell_metrics".to_string(),
                detail: "read-after-write decoded metric does not match input".to_string(),
            });
        }
        if let Some(counters) = &self.system_cost_counters {
            counters.record_rocksdb_write(value.len() as u64);
        }
        Ok(())
    }

    fn read_failure_fingerprint_catalog(
        &self,
    ) -> Result<Vec<FailureShapeFingerprint>, MejepaInferError> {
        let cf = cf(&self.db, CF_MEJEPA_FAILURE_FINGERPRINTS)?;
        let mut out = Vec::new();
        for item in self.db.iterator_cf(cf, IteratorMode::Start) {
            let (_key, value) = item?;
            let fingerprint: FailureShapeFingerprint = bincode::deserialize(&value)?;
            fingerprint.validate()?;
            out.push(fingerprint);
        }
        Ok(out)
    }

    fn read_latest_ood_calibration_report(
        &self,
    ) -> Result<Option<OodCalibrationReport>, MejepaInferError> {
        crate::ood_harvest::read_latest_ood_calibration_report(&self.db).map_err(|err| {
            MejepaInferError::InvalidInput {
                field: context_graph_mejepa_cf::CF_MEJEPA_OOD_CALIBRATIONS.to_string(),
                detail: err.to_string(),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calibration::open_infer_rocksdb;
    use crate::types::{
        AgentClaimGraph, ConformalInterval, ConformalSet, HierarchicalPredictionLevel,
        HierarchicalPredictionRecord, Language, OracleOutcome, PredictionHierarchyLevel,
        PredictionProvenance, RealityPrediction, ReasoningClass, TaskId, Verdict, WitnessHash,
        HIERARCHICAL_PREDICTION_SCHEMA_VERSION,
    };
    use std::collections::BTreeMap;

    fn prediction(session_id: [u8; 16]) -> RealityPrediction {
        RealityPrediction::try_new(RealityPrediction {
            prediction_id: [3; 16],
            witness_hash: WitnessHash([6; 32]),
            task_id: TaskId("store-test-task".to_string()),
            session_id,
            language: Language::Python,
            covered_chunks: Vec::new(),
            verdict: Verdict::Pass,
            confidence_interval: ConformalInterval {
                lower: 0.6,
                upper: 0.8,
                ..ConformalInterval::default()
            },
            predicted_oracle_pass: 0.9,
            predicted_test_pass: vec![0.9],
            predicted_runtime_trace: [0.0; 32],
            ood_score: 0.1,
            outcome_set: ConformalSet::try_new(vec![OracleOutcome::Pass], 0.1, 0.2).unwrap(),
            calibrated_confidence: 0.7,
            degraded_status: false,
            granger_attestations: BTreeMap::from([("skill:unit".to_string(), 0.9)]),
            predicted_failure_modes: Vec::new(),
            predicted_failed_tests: Vec::new(),
            predicted_works: Vec::new(),
            predicted_uncovered_paths: Vec::new(),
            predicted_flaky_tests: Vec::new(),
            guard_violations: Vec::new(),
            per_slot_ood_reasons: Vec::new(),
            closest_exemplars: Vec::new(),
            predicted_edge_cases: Vec::new(),
            predicted_latent_bugs: Vec::new(),
            predicted_tech_debt_added: Vec::new(),
            predicted_dead_code: Vec::new(),
            predicted_redundant_code: Vec::new(),
            predicted_perf_regressions: Vec::new(),
            predicted_security_concerns: Vec::new(),
            predicted_accuracy_degradations: Vec::new(),
            predicted_cost_regressions: Vec::new(),
            predicted_reasoning_class: ReasoningClass::Mute,
            agent_claim_graph: AgentClaimGraph::default(),
            claim_reconciliation: Vec::new(),
            reality_impact: None,
            provenance: PredictionProvenance {
                predictor_version: "store-test".to_string(),
                constellation_version: "store-test-constellation".to_string(),
                calibration_version: "calibration-v1".to_string(),
                active_pointer: hex::encode([3; 16]),
                train_health_source: String::new(),
            },
            source_panel_sha: [4; 32],
            calibration_version: "calibration-v1".to_string(),
            created_at_unix_ms: 1_772_000_000_000,
            matched_fingerprint: None,
            unknown_candidate_id: None,
            constellation_intelligence: None,
            slot_attributions: Vec::new(),
            label_context: Default::default(),
        })
        .unwrap()
    }

    #[test]
    fn live_prediction_write_reads_back_physical_row() {
        let temp = tempfile::tempdir().unwrap();
        let db = open_infer_rocksdb(temp.path()).unwrap();
        let store = RocksDbInferStore::new(db);
        let prediction = prediction([9; 16]);
        store.write_live_prediction(&prediction).unwrap();
        let loaded = store.read_live_predictions([9; 16], 1).unwrap();
        assert_eq!(loaded, vec![prediction]);
        assert!(store.session_known([9; 16]).unwrap());
        assert!(!store.session_known([8; 16]).unwrap());
    }

    #[test]
    fn hierarchical_prediction_write_reads_back_physical_row() {
        let temp = tempfile::tempdir().unwrap();
        let db = open_infer_rocksdb(temp.path()).unwrap();
        let store = RocksDbInferStore::new(db);
        let record = hierarchical_record([10; 16]);
        store.write_hierarchical_prediction(&record).unwrap();
        let loaded = store.read_hierarchical_predictions([10; 16], 1).unwrap();
        assert_eq!(loaded, vec![record]);
    }

    #[test]
    fn cold_cell_metric_write_reads_back_physical_row() {
        let temp = tempfile::tempdir().unwrap();
        let db = open_infer_rocksdb(temp.path()).unwrap();
        let store = RocksDbInferStore::new(db.clone());
        let metric = ColdCellMetric::try_new(
            "mutation=KnownGood:language=Python:entity=Function",
            "COLD_CELL_INSUFFICIENT_SUPPORT",
            Some(12),
            50,
            [7; 16],
            "task-cold-cell",
            [8; 16],
            1_778_000_000_000,
        )
        .unwrap();

        let cf = crate::calibration::cf(
            db.as_ref(),
            context_graph_mejepa_cf::CF_MEJEPA_COLD_CELL_METRICS,
        )
        .unwrap();
        let before = db.iterator_cf(cf, IteratorMode::Start).count();
        store.record_cold_cell_metric(&metric).unwrap();
        let after_rows = db
            .iterator_cf(cf, IteratorMode::Start)
            .map(|item| {
                let (_key, value) = item.unwrap();
                bincode::deserialize::<ColdCellMetric>(&value).unwrap()
            })
            .collect::<Vec<_>>();
        assert_eq!(after_rows.len(), before + 1);
        assert!(after_rows.contains(&metric));
    }

    #[test]
    fn live_prediction_limit_must_be_explicitly_bounded() {
        let temp = tempfile::tempdir().unwrap();
        let db = open_infer_rocksdb(temp.path()).unwrap();
        let store = RocksDbInferStore::new(db);
        assert_eq!(
            store.read_live_predictions([9; 16], 0).unwrap_err().code(),
            "MEJEPA_INFER_INVALID_INPUT"
        );
        assert_eq!(
            store
                .read_live_predictions([9; 16], 1001)
                .unwrap_err()
                .code(),
            "MEJEPA_INFER_INVALID_INPUT"
        );
    }

    fn hierarchical_record(session_id: [u8; 16]) -> HierarchicalPredictionRecord {
        let chunk = ChunkId("src/lib.py#0".to_string());
        HierarchicalPredictionRecord::try_new(HierarchicalPredictionRecord {
            schema_version: HIERARCHICAL_PREDICTION_SCHEMA_VERSION,
            prediction_id: [11; 16],
            task_id: TaskId("store-hierarchy-task".to_string()),
            session_id,
            language: Language::Python,
            source_panel_sha: [12; 32],
            calibration_version: "calibration-v1".to_string(),
            created_at_unix_ms: 1_772_000_000_001,
            slot_attributions: Vec::new(),
            levels: vec![
                HierarchicalPredictionLevel {
                    level: PredictionHierarchyLevel::File,
                    scope_id: "file:src/lib.py".to_string(),
                    parent_scope_id: None,
                    covered_chunks: vec![chunk.clone()],
                    predicted_oracle_pass: 0.9,
                    calibrated_confidence: 0.8,
                    ood_score: 0.1,
                    verdict: Verdict::Pass,
                    latent_energy: 0.01,
                },
                HierarchicalPredictionLevel {
                    level: PredictionHierarchyLevel::Function,
                    scope_id: "file:src/lib.py/function:answer#0".to_string(),
                    parent_scope_id: Some("file:src/lib.py".to_string()),
                    covered_chunks: vec![chunk.clone()],
                    predicted_oracle_pass: 0.9,
                    calibrated_confidence: 0.8,
                    ood_score: 0.1,
                    verdict: Verdict::Pass,
                    latent_energy: 0.01,
                },
                HierarchicalPredictionLevel {
                    level: PredictionHierarchyLevel::AstNode,
                    scope_id: "file:src/lib.py/function:answer#0/ast:function:abc".to_string(),
                    parent_scope_id: Some("file:src/lib.py/function:answer#0".to_string()),
                    covered_chunks: vec![chunk.clone()],
                    predicted_oracle_pass: 0.9,
                    calibrated_confidence: 0.8,
                    ood_score: 0.1,
                    verdict: Verdict::Pass,
                    latent_energy: 0.01,
                },
                HierarchicalPredictionLevel {
                    level: PredictionHierarchyLevel::Chunk,
                    scope_id: "file:src/lib.py/function:answer#0/ast:function:abc/chunk:0"
                        .to_string(),
                    parent_scope_id: Some(
                        "file:src/lib.py/function:answer#0/ast:function:abc".to_string(),
                    ),
                    covered_chunks: vec![chunk],
                    predicted_oracle_pass: 0.9,
                    calibrated_confidence: 0.8,
                    ood_score: 0.1,
                    verdict: Verdict::Pass,
                    latent_energy: 0.01,
                },
            ],
        })
        .unwrap()
    }
}
