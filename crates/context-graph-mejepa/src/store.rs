use std::sync::Arc;

use rocksdb::{Direction, IteratorMode, WriteOptions, DB};

use crate::calibration::{
    cf, CF_MEJEPA_LIVE_PREDICTIONS, CF_MEJEPA_ORACLE_VERDICTS, CF_MEJEPA_TRAIN_CERTS,
};
use crate::compiler::{MejepaStore, TrainCertSummary};
use crate::conformal::CalibrationExample;
use crate::error::MejepaInferError;
use crate::park_list::{
    clear_park_list_entry as clear_park_list_entry_row,
    read_park_list_entry as read_park_list_entry_row,
    record_park_list_failure as record_park_list_failure_row, ParkListEntry,
};
use crate::types::{ChunkId, DdaSignals, PanelId, RealityPrediction};

#[derive(Clone)]
pub struct RocksDbInferStore {
    db: Arc<DB>,
}

impl RocksDbInferStore {
    pub fn new(db: Arc<DB>) -> Self {
        Self { db }
    }

    pub fn db(&self) -> Arc<DB> {
        self.db.clone()
    }

    fn live_key(prediction: &RealityPrediction) -> Vec<u8> {
        let mut key = Vec::with_capacity(40);
        key.extend_from_slice(&prediction.session_id);
        key.extend_from_slice(&prediction.created_at_unix_ms.to_be_bytes());
        key.extend_from_slice(&prediction.prediction_id);
        key
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
        let decoded: RealityPrediction = bincode::deserialize(&readback)?;
        decoded.validate()?;
        if decoded != *prediction {
            return Err(MejepaInferError::InvalidInput {
                field: "live_predictions".to_string(),
                detail: "read-after-write decoded prediction does not match input".to_string(),
            });
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
            let prediction: RealityPrediction = bincode::deserialize(&value)?;
            prediction.validate()?;
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
        let prediction: RealityPrediction = bincode::deserialize(&value)?;
        prediction.validate()?;
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
        record_park_list_failure_row(&self.db, prediction_id, now_unix_ms, error_code)
    }

    fn clear_park_list_entry(&self, prediction_id: [u8; 16]) -> Result<(), MejepaInferError> {
        clear_park_list_entry_row(&self.db, prediction_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calibration::open_infer_rocksdb;
    use crate::types::{
        AgentClaimGraph, ConformalInterval, ConformalSet, Language, OracleOutcome,
        PredictionProvenance, RealityPrediction, ReasoningClass, TaskId, Verdict, WitnessHash,
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
            guard_violations: Vec::new(),
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
            },
            source_panel_sha: [4; 32],
            calibration_version: "calibration-v1".to_string(),
            created_at_unix_ms: 1_772_000_000_000,
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
}
