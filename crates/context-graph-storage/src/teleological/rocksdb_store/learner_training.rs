//! Learner training matrix dataset persistence.
//!
//! Records are stored in `CF_LEARNER_TRAINING_DATASETS` as
//! `[LEARNER_TRAINING_DATASET_VERSION: u8][bincode LearnerTrainingDataset]`.

use context_graph_core::error::{CoreError, CoreResult};
use context_graph_core::learner_training::{
    LearnerTrainingDataset, LEARNER_TRAINING_DATASET_VERSION,
};
use rocksdb::{ColumnFamily, IteratorMode};
use tracing::{debug, error};
use uuid::Uuid;

use crate::teleological::column_families::CF_LEARNER_TRAINING_DATASETS;

use super::store::RocksDbTeleologicalStore;
use super::types::TeleologicalStoreError;

impl RocksDbTeleologicalStore {
    #[inline]
    fn cf_learner_training_datasets(&self) -> &ColumnFamily {
        self.db
            .cf_handle(CF_LEARNER_TRAINING_DATASETS)
            .expect("CF_LEARNER_TRAINING_DATASETS must exist — database initialization failed")
    }

    pub async fn store_learner_training_dataset(
        &self,
        dataset: &LearnerTrainingDataset,
    ) -> CoreResult<()> {
        let payload = encode_learner_training_dataset(dataset)?;
        let cf = self.cf_learner_training_datasets();
        self.db
            .put_cf(cf, dataset.dataset_id.as_bytes(), &payload)
            .map_err(|e| {
                error!(
                    dataset_id = %dataset.dataset_id,
                    error = %e,
                    "ROCKSDB ERROR: Failed to store learner training dataset"
                );
                TeleologicalStoreError::rocksdb_op(
                    "put_learner_training_dataset",
                    CF_LEARNER_TRAINING_DATASETS,
                    Some(dataset.dataset_id),
                    e,
                )
            })?;
        debug!(
            dataset_id = %dataset.dataset_id,
            rows = dataset.rows_len,
            cols = dataset.cols_len,
            bytes = payload.len(),
            "Stored learner training dataset"
        );
        Ok(())
    }

    pub async fn get_learner_training_dataset(
        &self,
        dataset_id: Uuid,
    ) -> CoreResult<Option<LearnerTrainingDataset>> {
        let cf = self.cf_learner_training_datasets();
        match self.db.get_cf(cf, dataset_id.as_bytes()) {
            Ok(Some(bytes)) => decode_learner_training_dataset(&bytes).map(Some),
            Ok(None) => Ok(None),
            Err(e) => {
                error!(
                    dataset_id = %dataset_id,
                    error = %e,
                    "ROCKSDB ERROR: Failed to read learner training dataset"
                );
                Err(TeleologicalStoreError::rocksdb_op(
                    "get_learner_training_dataset",
                    CF_LEARNER_TRAINING_DATASETS,
                    Some(dataset_id),
                    e,
                )
                .into())
            }
        }
    }

    pub async fn list_learner_training_dataset_ids(&self) -> CoreResult<Vec<Uuid>> {
        let cf = self.cf_learner_training_datasets();
        let iter = self.db.iterator_cf(cf, IteratorMode::Start);
        let mut out = Vec::new();
        for item in iter {
            let (key, _) = item.map_err(|e| {
                error!(
                    error = %e,
                    "ROCKSDB ERROR: iteration failed in list_learner_training_dataset_ids"
                );
                TeleologicalStoreError::rocksdb_op(
                    "iterate_learner_training_datasets",
                    CF_LEARNER_TRAINING_DATASETS,
                    None,
                    e,
                )
            })?;
            if key.len() == 16 {
                let mut buf = [0u8; 16];
                buf.copy_from_slice(&key);
                out.push(Uuid::from_bytes(buf));
            }
        }
        Ok(out)
    }

    pub async fn count_learner_training_datasets(&self) -> CoreResult<usize> {
        let cf = self.cf_learner_training_datasets();
        Ok(self.db.iterator_cf(cf, IteratorMode::Start).count())
    }

    pub async fn clear_all_learner_training_datasets(&self) -> CoreResult<usize> {
        let ids = self.list_learner_training_dataset_ids().await?;
        let cf = self.cf_learner_training_datasets();
        for id in &ids {
            self.db.delete_cf(cf, id.as_bytes()).map_err(|e| {
                error!(
                    dataset_id = %id,
                    error = %e,
                    "ROCKSDB ERROR: Failed to delete learner training dataset during clear"
                );
                TeleologicalStoreError::rocksdb_op(
                    "delete_learner_training_dataset",
                    CF_LEARNER_TRAINING_DATASETS,
                    Some(*id),
                    e,
                )
            })?;
        }
        Ok(ids.len())
    }
}

pub fn encode_learner_training_dataset(dataset: &LearnerTrainingDataset) -> CoreResult<Vec<u8>> {
    dataset.validate()?;
    let mut bytes = bincode::serialize(dataset).map_err(|e| {
        CoreError::SerializationError(format!("bincode serialize LearnerTrainingDataset: {e}"))
    })?;
    let mut out = Vec::with_capacity(1 + bytes.len());
    out.push(LEARNER_TRAINING_DATASET_VERSION);
    out.append(&mut bytes);
    Ok(out)
}

pub fn decode_learner_training_dataset(bytes: &[u8]) -> CoreResult<LearnerTrainingDataset> {
    if bytes.is_empty() {
        return Err(CoreError::SerializationError(
            "learner training dataset payload is empty (missing version byte)".into(),
        ));
    }
    if bytes[0] != LEARNER_TRAINING_DATASET_VERSION {
        return Err(CoreError::SerializationError(format!(
            "learner training dataset version mismatch: got {}, expected {}. No automatic migration is supported.",
            bytes[0], LEARNER_TRAINING_DATASET_VERSION
        )));
    }
    let dataset: LearnerTrainingDataset = bincode::deserialize(&bytes[1..]).map_err(|e| {
        CoreError::SerializationError(format!("bincode deserialize LearnerTrainingDataset: {e}"))
    })?;
    dataset.validate()?;
    Ok(dataset)
}
