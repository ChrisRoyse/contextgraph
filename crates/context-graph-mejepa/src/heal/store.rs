use std::path::Path;
use std::sync::Arc;

use rocksdb::{ColumnFamilyDescriptor, IteratorMode, Options, DB};

use crate::heal::cf::{
    all_referenced_cf_names, encode_calibration_history_key, encode_drift_history_key,
    encode_drift_window_key, encode_fisher_snapshot_key, CF_MEJEPA_CALIBRATION_HISTORY,
    CF_MEJEPA_DRIFT_HISTORY, CF_MEJEPA_DRIFT_WINDOW, CF_MEJEPA_FISHER_SNAPSHOTS,
    CF_MEJEPA_TRAIN_CERTS,
};
use crate::heal::cf::{DriftHistoryRecord, encode_value};
use crate::heal::drift::{DriftHistoryStore, DriftSample, DriftStore};
use crate::heal::errors::HealError;
use crate::heal::ewc::{SnapshotStore, TaskSnapshot};

#[derive(Clone)]
pub struct HealRocksStore {
    db: Arc<DB>,
}

impl HealRocksStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Arc<Self>, HealError> {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);
        opts.set_paranoid_checks(true);
        let descriptors = all_referenced_cf_names()
            .iter()
            .map(|name| ColumnFamilyDescriptor::new(*name, Options::default()))
            .collect::<Vec<_>>();
        let db = Arc::new(DB::open_cf_descriptors(&opts, path.as_ref(), descriptors)?);
        for cf in all_referenced_cf_names() {
            if db.cf_handle(cf).is_none() {
                return Err(HealError::invalid(
                    "heal_store.column_family",
                    format!("missing column family {cf}"),
                ));
            }
        }
        Ok(Arc::new(Self { db }))
    }

    pub fn from_db(db: Arc<DB>) -> Result<Arc<Self>, HealError> {
        for cf in all_referenced_cf_names() {
            if db.cf_handle(cf).is_none() {
                return Err(HealError::invalid(
                    "heal_store.column_family",
                    format!("missing column family {cf}"),
                ));
            }
        }
        Ok(Arc::new(Self { db }))
    }

    pub fn db(&self) -> Arc<DB> {
        self.db.clone()
    }

    pub fn put_cf_readback(&self, cf_name: &str, key: &[u8], value: &[u8]) -> Result<(), HealError> {
        let cf = self.cf(cf_name)?;
        self.db.put_cf(cf, key, value)?;
        let readback = self.db.get_cf(cf, key)?.ok_or_else(|| {
            HealError::invalid(
                "heal_store.readback",
                format!("missing row after put in {cf_name}"),
            )
        })?;
        if readback.as_ref() != value {
            return Err(HealError::invalid(
                "heal_store.readback",
                format!("readback mismatch in {cf_name}"),
            ));
        }
        Ok(())
    }

    pub fn get_cf(&self, cf_name: &str, key: &[u8]) -> Result<Option<Vec<u8>>, HealError> {
        Ok(self.db.get_cf(self.cf(cf_name)?, key)?)
    }

    pub fn count_cf(&self, cf_name: &str) -> Result<u64, HealError> {
        let mut count = 0u64;
        for item in self.db.iterator_cf(self.cf(cf_name)?, IteratorMode::Start) {
            let _ = item?;
            count += 1;
        }
        Ok(count)
    }

    pub fn latest_train_cert_means(
        &self,
        limit: usize,
    ) -> Result<Option<(f32, f32, usize)>, HealError> {
        if limit == 0 {
            return Ok(None);
        }
        let cf = self.cf(CF_MEJEPA_TRAIN_CERTS)?;
        let mut delta_omega = 0.0f32;
        let mut delta_xi = 0.0f32;
        let mut count = 0usize;
        for item in self.db.iterator_cf(cf, IteratorMode::End) {
            let (_key, value) = item?;
            if let Ok(summary) = bincode::deserialize::<crate::compiler::TrainCertSummary>(&value) {
                summary.validate().map_err(|err| {
                    HealError::invalid("train_cert.summary", format!("{}: {err}", err.code()))
                })?;
                delta_omega += summary.delta_omega;
                delta_xi += summary.delta_xi;
                count += 1;
            }
            if count == limit {
                break;
            }
        }
        if count == 0 {
            Ok(None)
        } else {
            Ok(Some((
                delta_omega / count as f32,
                delta_xi / count as f32,
                count,
            )))
        }
    }

    fn cf<'a>(&'a self, name: &str) -> Result<&'a rocksdb::ColumnFamily, HealError> {
        self.db.cf_handle(name).ok_or_else(|| {
            HealError::invalid(
                "heal_store.column_family",
                format!("missing column family {name}"),
            )
        })
    }
}

impl SnapshotStore for HealRocksStore {
    fn persist_task_snapshot(&self, snapshot: &TaskSnapshot) -> Result<(), HealError> {
        self.put_cf_readback(
            CF_MEJEPA_FISHER_SNAPSHOTS,
            &encode_fisher_snapshot_key(snapshot.boundary_step),
            &encode_value(snapshot)?,
        )
    }
}

impl DriftStore for HealRocksStore {
    fn persist_drift_sample(&self, offset: u64, sample: &DriftSample) -> Result<(), HealError> {
        self.put_cf_readback(
            CF_MEJEPA_DRIFT_WINDOW,
            &encode_drift_window_key(offset),
            &encode_value(sample)?,
        )
    }
}

impl DriftHistoryStore for HealRocksStore {
    fn persist_drift_history(&self, record: &DriftHistoryRecord) -> Result<(), HealError> {
        self.put_cf_readback(
            CF_MEJEPA_DRIFT_HISTORY,
            &encode_drift_history_key(chrono::Utc::now().timestamp()),
            &encode_value(record)?,
        )
    }
}

pub fn persist_calibration_bytes(
    storage: &HealRocksStore,
    frozen_at: i64,
    bytes: &[u8],
) -> Result<(), HealError> {
    storage.put_cf_readback(
        CF_MEJEPA_CALIBRATION_HISTORY,
        &encode_calibration_history_key(frozen_at),
        bytes,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::heal::cf::CF_MEJEPA_WEIGHT_BLOBS;

    #[test]
    fn put_cf_readback_persists_physical_row() {
        let temp = tempfile::tempdir().unwrap();
        let store = HealRocksStore::open(temp.path()).unwrap();
        store
            .put_cf_readback(CF_MEJEPA_WEIGHT_BLOBS, b"k", b"v")
            .unwrap();
        assert_eq!(store.get_cf(CF_MEJEPA_WEIGHT_BLOBS, b"k").unwrap(), Some(b"v".to_vec()));
        assert_eq!(store.count_cf(CF_MEJEPA_WEIGHT_BLOBS).unwrap(), 1);
    }
}
