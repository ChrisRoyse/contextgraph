use std::sync::Arc;

use rocksdb::{Direction, IteratorMode, DB};

use crate::calibration::{
    cf, CF_MEJEPA_LIVE_PREDICTIONS, CF_MEJEPA_ORACLE_VERDICTS, CF_MEJEPA_TRAIN_CERTS,
};
use crate::compiler::{MejepaStore, TrainCertSummary};
use crate::conformal::CalibrationExample;
use crate::error::MejepaInferError;
use crate::types::RealityPrediction;

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
        self.db.put_cf(cf, &key, &value)?;
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calibration::open_infer_rocksdb;
    use crate::types::{ConformalSet, Language, OracleOutcome, RealityPrediction, TaskId};
    use std::collections::BTreeMap;

    fn prediction(session_id: [u8; 16]) -> RealityPrediction {
        RealityPrediction::try_new(RealityPrediction {
            prediction_id: [3; 16],
            task_id: TaskId("store-test-task".to_string()),
            session_id,
            language: Language::Python,
            predicted_oracle_pass: 0.9,
            predicted_test_pass: vec![0.9],
            predicted_runtime_trace: [0.0; 32],
            ood_score: 0.1,
            outcome_set: ConformalSet::try_new(vec![OracleOutcome::Pass], 0.1, 0.2).unwrap(),
            calibrated_confidence: 0.7,
            degraded_status: false,
            granger_attestations: BTreeMap::from([("skill:unit".to_string(), 0.9)]),
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
