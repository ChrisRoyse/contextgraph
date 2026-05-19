use context_graph_mejepa_cf::CF_MEJEPA_Q4_ACCURACY_LABELS;
use rocksdb::{ColumnFamilyDescriptor, IteratorMode, Options, WriteOptions, DB};

use super::*;

pub struct Q4AccuracyLabelStore {
    db: DB,
}

impl Q4AccuracyLabelStore {
    pub fn open(path: impl AsRef<Path>) -> InstrumentResult<Self> {
        let mut db_opts = Options::default();
        db_opts.create_if_missing(true);
        db_opts.create_missing_column_families(true);
        db_opts.set_paranoid_checks(true);
        let descriptors = vec![
            ColumnFamilyDescriptor::new("default", Options::default()),
            ColumnFamilyDescriptor::new(CF_MEJEPA_Q4_ACCURACY_LABELS, cf_options()),
        ];
        let db = DB::open_cf_descriptors(&db_opts, path.as_ref(), descriptors).map_err(|err| {
            InstrumentError::store(
                "open",
                CF_MEJEPA_Q4_ACCURACY_LABELS,
                err.to_string(),
                "inspect the RocksDB path, lock ownership, and column-family metadata",
            )
        })?;
        Ok(Self { db })
    }

    pub fn put_extraction(
        &self,
        extraction: &Q4AccuracyExtraction,
    ) -> InstrumentResult<Vec<String>> {
        let mut keys = Vec::new();
        for label in &extraction.labels {
            keys.push(self.put_record(&PersistedQ4AccuracySignal {
                schema_version: Q4_ACCURACY_SCHEMA_VERSION,
                signal: Q4AccuracySignalRecord::Label(label.clone()),
            })?);
        }
        for quarantine in &extraction.quarantines {
            keys.push(self.put_record(&PersistedQ4AccuracySignal {
                schema_version: Q4_ACCURACY_SCHEMA_VERSION,
                signal: Q4AccuracySignalRecord::Quarantine(quarantine.clone()),
            })?);
        }
        Ok(keys)
    }

    pub fn scan_records(&self) -> InstrumentResult<Vec<(String, PersistedQ4AccuracySignal)>> {
        let cf = self.cf()?;
        let mut rows = Vec::new();
        for item in self.db.iterator_cf(cf, IteratorMode::Start) {
            let (key, value) = item.map_err(|err| {
                InstrumentError::store(
                    "iterate",
                    CF_MEJEPA_Q4_ACCURACY_LABELS,
                    err.to_string(),
                    "inspect RocksDB iterator state and Q4 accuracy CF health",
                )
            })?;
            let key = String::from_utf8(key.to_vec()).map_err(|err| {
                InstrumentError::store(
                    "decode_key",
                    CF_MEJEPA_Q4_ACCURACY_LABELS,
                    err.to_string(),
                    "Q4 accuracy signal keys must be UTF-8",
                )
            })?;
            let record = serde_json::from_slice(&value).map_err(|err| {
                InstrumentError::store(
                    "deserialize",
                    CF_MEJEPA_Q4_ACCURACY_LABELS,
                    err.to_string(),
                    "only mutate Q4 accuracy rows through Q4AccuracyLabelStore",
                )
            })?;
            rows.push((key, record));
        }
        Ok(rows)
    }

    pub fn get_label_record(
        &self,
        corpus_row_id: &str,
        metric_name: &str,
    ) -> InstrumentResult<Option<PersistedQ4AccuracySignal>> {
        validate_path_component("corpus_row_id", corpus_row_id)?;
        validate_non_empty_single_line("q4_accuracy.label.metric_name", metric_name)?;
        let mut matches = self
            .scan_records()?
            .into_iter()
            .filter_map(|(_, record)| match &record.signal {
                Q4AccuracySignalRecord::Label(label)
                    if label.corpus_row_id == corpus_row_id && label.metric_name == metric_name =>
                {
                    Some(record)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        if matches.len() > 1 {
            return Err(InstrumentError::store(
                "get_label_record",
                CF_MEJEPA_Q4_ACCURACY_LABELS,
                format!(
                    "metric {metric_name} for row {corpus_row_id} has {} source-test records",
                    matches.len()
                ),
                "read duplicate metric rows with scan_records and source_test-aware filtering",
            ));
        }
        Ok(matches.pop())
    }

    pub fn count_records(&self) -> InstrumentResult<usize> {
        Ok(self.scan_records()?.len())
    }

    pub fn flush(&self) -> InstrumentResult<()> {
        self.db.flush_cf(self.cf()?).map_err(|err| {
            InstrumentError::store(
                "flush",
                CF_MEJEPA_Q4_ACCURACY_LABELS,
                err.to_string(),
                "inspect RocksDB WAL and filesystem state",
            )
        })
    }

    fn put_record(&self, record: &PersistedQ4AccuracySignal) -> InstrumentResult<String> {
        let value = serde_json::to_vec(record).map_err(|err| {
            InstrumentError::store(
                "serialize",
                CF_MEJEPA_Q4_ACCURACY_LABELS,
                err.to_string(),
                "ensure Q4 accuracy records remain JSON-serializable",
            )
        })?;
        let key = q4_accuracy_record_key(record, &value);
        let mut write_opts = WriteOptions::default();
        write_opts.set_sync(true);
        self.db
            .put_cf_opt(self.cf()?, key.as_bytes(), &value, &write_opts)
            .map_err(|err| {
                InstrumentError::store(
                    "put_cf",
                    CF_MEJEPA_Q4_ACCURACY_LABELS,
                    err.to_string(),
                    "inspect RocksDB write permissions, WAL state, and disk capacity",
                )
            })?;
        let readback = self.db.get_cf(self.cf()?, key.as_bytes()).map_err(|err| {
            InstrumentError::store(
                "get_cf",
                CF_MEJEPA_Q4_ACCURACY_LABELS,
                err.to_string(),
                "inspect RocksDB read permissions and column-family health",
            )
        })?;
        if readback.as_deref() != Some(value.as_slice()) {
            return Err(InstrumentError::store(
                "read_after_write",
                CF_MEJEPA_Q4_ACCURACY_LABELS,
                "Q4 accuracy row missing or changed after put_cf",
                "do not advance reward-signal checkpoints until the CF row is readable",
            ));
        }
        Ok(key)
    }

    fn cf(&self) -> InstrumentResult<&rocksdb::ColumnFamily> {
        self.db
            .cf_handle(CF_MEJEPA_Q4_ACCURACY_LABELS)
            .ok_or_else(|| {
                InstrumentError::store(
                    "cf_handle",
                    CF_MEJEPA_Q4_ACCURACY_LABELS,
                    "column-family handle not found",
                    "open the store through Q4AccuracyLabelStore::open",
                )
            })
    }
}
