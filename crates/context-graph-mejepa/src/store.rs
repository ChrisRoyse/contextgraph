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
        let Some(cf) = self.db.cf_handle(CF_MEJEPA_TRAIN_CERTS) else {
            return Ok(Vec::new());
        };
        let mut out = Vec::with_capacity(limit);
        for item in self.db.iterator_cf(cf, IteratorMode::End) {
            let (_key, value) = item?;
            let cert: TrainCertSummary = bincode::deserialize(&value)?;
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
        let cf = cf(&self.db, CF_MEJEPA_LIVE_PREDICTIONS)?;
        self.db.put_cf(
            cf,
            Self::live_key(prediction),
            bincode::serialize(prediction)?,
        )?;
        Ok(())
    }

    fn read_live_predictions(
        &self,
        session_id: [u8; 16],
        limit: u32,
    ) -> Result<Vec<RealityPrediction>, MejepaInferError> {
        let limit = limit.clamp(1, 1000) as usize;
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
            let prediction: RealityPrediction = bincode::deserialize(&value)?;
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
