use std::path::Path;
use std::sync::Arc;

use rocksdb::{ColumnFamilyDescriptor, IteratorMode, Options, WriteOptions, DB};

use crate::heal::cf::{
    all_referenced_cf_names, encode_calibration_history_key, encode_drift_history_key,
    encode_drift_window_key, encode_fisher_snapshot_key, CF_MEJEPA_CALIBRATION_HISTORY,
    CF_MEJEPA_DRIFT_HISTORY, CF_MEJEPA_DRIFT_WINDOW, CF_MEJEPA_FISHER_SNAPSHOTS,
    CF_MEJEPA_TRAIN_CERTS,
};
use crate::heal::cf::{encode_value, DriftHistoryRecord};
use crate::heal::drift::{DriftHistoryStore, DriftSample, DriftStore};
use crate::heal::errors::HealError;
use crate::heal::ewc::{SnapshotStore, TaskSnapshot};
use crate::heal::integrity::{active_witness_quarantine, is_witness_quarantine_pointer_key};
use crate::system_cost::SystemCostCounters;

#[derive(Clone)]
pub struct HealRocksStore {
    db: Arc<DB>,
    system_cost_counters: Option<Arc<SystemCostCounters>>,
}

impl HealRocksStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Arc<Self>, HealError> {
        Self::open_inner(path, None)
    }

    pub fn open_with_system_cost_counters(
        path: impl AsRef<Path>,
        system_cost_counters: Arc<SystemCostCounters>,
    ) -> Result<Arc<Self>, HealError> {
        Self::open_inner(path, Some(system_cost_counters))
    }

    fn open_inner(
        path: impl AsRef<Path>,
        system_cost_counters: Option<Arc<SystemCostCounters>>,
    ) -> Result<Arc<Self>, HealError> {
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
        Ok(Arc::new(Self {
            db,
            system_cost_counters,
        }))
    }

    pub fn from_db(db: Arc<DB>) -> Result<Arc<Self>, HealError> {
        Self::from_db_inner(db, None)
    }

    pub fn from_db_with_system_cost_counters(
        db: Arc<DB>,
        system_cost_counters: Arc<SystemCostCounters>,
    ) -> Result<Arc<Self>, HealError> {
        Self::from_db_inner(db, Some(system_cost_counters))
    }

    fn from_db_inner(
        db: Arc<DB>,
        system_cost_counters: Option<Arc<SystemCostCounters>>,
    ) -> Result<Arc<Self>, HealError> {
        for cf in all_referenced_cf_names() {
            if db.cf_handle(cf).is_none() {
                return Err(HealError::invalid(
                    "heal_store.column_family",
                    format!("missing column family {cf}"),
                ));
            }
        }
        Ok(Arc::new(Self {
            db,
            system_cost_counters,
        }))
    }

    pub fn db(&self) -> Arc<DB> {
        self.db.clone()
    }

    pub fn system_cost_counters(&self) -> Option<Arc<SystemCostCounters>> {
        self.system_cost_counters.clone()
    }

    pub fn put_cf_readback(
        &self,
        cf_name: &str,
        key: &[u8],
        value: &[u8],
    ) -> Result<(), HealError> {
        self.ensure_not_witness_quarantined_for_write(cf_name, key)?;
        let cf = self.cf(cf_name)?;
        let mut opts = WriteOptions::default();
        opts.set_sync(true);
        self.db.put_cf_opt(cf, key, value, &opts)?;
        let readback = self.db.get_cf(cf, key)?.ok_or_else(|| {
            HealError::invalid(
                "heal_store.readback",
                format!("missing row after put in {cf_name}"),
            )
        })?;
        if readback.as_slice() != value {
            return Err(HealError::invalid(
                "heal_store.readback",
                format!("readback mismatch in {cf_name}"),
            ));
        }
        if let Some(counters) = &self.system_cost_counters {
            counters.record_rocksdb_write(value.len() as u64);
        }
        Ok(())
    }

    fn ensure_not_witness_quarantined_for_write(
        &self,
        cf_name: &str,
        key: &[u8],
    ) -> Result<(), HealError> {
        if Self::write_allowed_during_witness_quarantine(cf_name, key) {
            return Ok(());
        }
        if let Some(record) = active_witness_quarantine(self)? {
            return Err(HealError::WitnessQuarantined {
                reason: record.reason,
                repair_promotion_id: record.repair_promotion_id,
            });
        }
        Ok(())
    }

    fn write_allowed_during_witness_quarantine(cf_name: &str, key: &[u8]) -> bool {
        if cf_name == context_graph_mejepa_cf::CF_MEJEPA_ACTIVE_POINTERS {
            return is_witness_quarantine_pointer_key(key);
        }
        if cf_name == context_graph_mejepa_cf::CF_MEJEPA_MODEL_PROMOTIONS {
            return key.starts_with(b"phase_e/pending-promotion/")
                || key.starts_with(b"phase_e/operator-alert/");
        }
        false
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

    pub fn scan_cf_values(&self, cf_name: &str) -> Result<Vec<Vec<u8>>, HealError> {
        let mut values = Vec::new();
        for item in self.db.iterator_cf(self.cf(cf_name)?, IteratorMode::Start) {
            let (_key, value) = item?;
            values.push(value.to_vec());
        }
        Ok(values)
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
    fn replace_task_snapshots(&self, snapshots: &[TaskSnapshot]) -> Result<(), HealError> {
        let cf = self.cf(CF_MEJEPA_FISHER_SNAPSHOTS)?;
        let existing = self
            .db
            .iterator_cf(cf, IteratorMode::Start)
            .map(|item| item.map(|(key, _value)| key.to_vec()))
            .collect::<Result<Vec<_>, _>>()?;
        for key in existing {
            self.db.delete_cf(cf, key)?;
        }
        for snapshot in snapshots {
            self.put_cf_readback(
                CF_MEJEPA_FISHER_SNAPSHOTS,
                &encode_fisher_snapshot_key(snapshot.boundary_step),
                &encode_value(snapshot)?,
            )?;
        }
        let count = self.count_cf(CF_MEJEPA_FISHER_SNAPSHOTS)? as usize;
        if count != snapshots.len() {
            return Err(HealError::invalid(
                "heal_store.fisher_snapshot_retention",
                format!(
                    "expected {} persisted snapshots, got {count}",
                    snapshots.len()
                ),
            ));
        }
        Ok(())
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
        assert_eq!(
            store.get_cf(CF_MEJEPA_WEIGHT_BLOBS, b"k").unwrap(),
            Some(b"v".to_vec())
        );
        assert_eq!(store.count_cf(CF_MEJEPA_WEIGHT_BLOBS).unwrap(), 1);
    }

    #[test]
    fn replace_task_snapshots_matches_persisted_source_of_truth() {
        let temp = tempfile::tempdir().unwrap();
        let store = HealRocksStore::open(temp.path()).unwrap();
        let first = (0..7)
            .map(|idx| {
                TaskSnapshot::try_new(
                    vec![idx as f32],
                    vec![1.0],
                    idx,
                    [idx as u8; 32],
                    idx as i64,
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        store.replace_task_snapshots(&first).unwrap();
        assert_eq!(store.count_cf(CF_MEJEPA_FISHER_SNAPSHOTS).unwrap(), 7);
        let retained = first[2..].to_vec();
        store.replace_task_snapshots(&retained).unwrap();
        assert_eq!(store.count_cf(CF_MEJEPA_FISHER_SNAPSHOTS).unwrap(), 5);
    }
}
