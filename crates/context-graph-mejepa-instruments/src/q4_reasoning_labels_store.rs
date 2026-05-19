use context_graph_mejepa_cf::{CF_MEJEPA_HEAD_CALIBRATIONS, CF_MEJEPA_Q4_REASONING_LABELS};
use rocksdb::{ColumnFamilyDescriptor, IteratorMode, Options, WriteOptions, DB};

use super::*;

pub struct Q4ReasoningLabelStore {
    db: DB,
}

impl Q4ReasoningLabelStore {
    pub fn open(path: impl AsRef<Path>) -> InstrumentResult<Self> {
        let mut db_opts = Options::default();
        db_opts.create_if_missing(true);
        db_opts.create_missing_column_families(true);
        db_opts.set_paranoid_checks(true);
        let descriptors = vec![
            ColumnFamilyDescriptor::new("default", Options::default()),
            ColumnFamilyDescriptor::new(CF_MEJEPA_Q4_REASONING_LABELS, cf_options()),
            ColumnFamilyDescriptor::new(CF_MEJEPA_HEAD_CALIBRATIONS, cf_options()),
        ];
        let db = DB::open_cf_descriptors(&db_opts, path.as_ref(), descriptors).map_err(|err| {
            InstrumentError::store(
                "open",
                CF_MEJEPA_Q4_REASONING_LABELS,
                err.to_string(),
                "inspect the RocksDB path, lock ownership, and column-family metadata",
            )
        })?;
        Ok(Self { db })
    }

    pub fn put_extraction(
        &self,
        extraction: &Q4ReasoningExtraction,
    ) -> InstrumentResult<Vec<String>> {
        let mut keys = Vec::new();
        for label in &extraction.labels {
            let record = PersistedQ4ReasoningSignal {
                schema_version: Q4_REASONING_SCHEMA_VERSION,
                signal: Q4ReasoningSignalRecord::Label(label.clone()),
            };
            keys.push(self.put_signal_record(&record)?);
        }
        for quarantine in &extraction.quarantines {
            let record = PersistedQ4ReasoningSignal {
                schema_version: Q4_REASONING_SCHEMA_VERSION,
                signal: Q4ReasoningSignalRecord::Quarantine(quarantine.clone()),
            };
            keys.push(self.put_signal_record(&record)?);
        }
        Ok(keys)
    }

    pub fn put_calibration_rows(
        &self,
        rows: &[Q4ReasoningCalibrationRow],
    ) -> InstrumentResult<Vec<String>> {
        let mut keys = Vec::new();
        for row in rows {
            validate_calibration_row(row)?;
            let record = PersistedQ4ReasoningCalibration {
                schema_version: Q4_REASONING_SCHEMA_VERSION,
                row: row.clone(),
            };
            keys.push(self.put_calibration_record(&record)?);
        }
        Ok(keys)
    }

    pub fn scan_records(&self) -> InstrumentResult<Vec<(String, PersistedQ4ReasoningSignal)>> {
        let cf = self.signal_cf()?;
        let mut rows = Vec::new();
        for item in self.db.iterator_cf(cf, IteratorMode::Start) {
            let (key, value) = item.map_err(|err| {
                InstrumentError::store(
                    "iterate",
                    CF_MEJEPA_Q4_REASONING_LABELS,
                    err.to_string(),
                    "inspect RocksDB iterator state and Q4 reasoning CF health",
                )
            })?;
            rows.push((
                decode_key(CF_MEJEPA_Q4_REASONING_LABELS, &key)?,
                serde_json::from_slice(&value).map_err(|err| {
                    InstrumentError::store(
                        "deserialize",
                        CF_MEJEPA_Q4_REASONING_LABELS,
                        err.to_string(),
                        "only mutate Q4 reasoning rows through Q4ReasoningLabelStore",
                    )
                })?,
            ));
        }
        Ok(rows)
    }

    pub fn scan_calibrations(
        &self,
    ) -> InstrumentResult<Vec<(String, PersistedQ4ReasoningCalibration)>> {
        let cf = self.calibration_cf()?;
        let mut rows = Vec::new();
        for item in self.db.iterator_cf(cf, IteratorMode::Start) {
            let (key, value) = item.map_err(|err| {
                InstrumentError::store(
                    "iterate",
                    CF_MEJEPA_HEAD_CALIBRATIONS,
                    err.to_string(),
                    "inspect RocksDB iterator state and head-calibration CF health",
                )
            })?;
            let key = decode_key(CF_MEJEPA_HEAD_CALIBRATIONS, &key)?;
            if !key.starts_with("reasoning::") {
                continue;
            }
            rows.push((
                key,
                serde_json::from_slice(&value).map_err(|err| {
                    InstrumentError::store(
                        "deserialize",
                        CF_MEJEPA_HEAD_CALIBRATIONS,
                        err.to_string(),
                        "only mutate head calibration rows through Q4ReasoningLabelStore",
                    )
                })?,
            ));
        }
        Ok(rows)
    }

    pub fn get_label_record(
        &self,
        corpus_row_id: &str,
        session_id: &str,
    ) -> InstrumentResult<Option<PersistedQ4ReasoningSignal>> {
        validate_path_component("corpus_row_id", corpus_row_id)?;
        validate_non_empty_single_line("session_id", session_id)?;
        let key = q4_reasoning_label_key(corpus_row_id, session_id);
        let Some(value) = self
            .db
            .get_cf(self.signal_cf()?, key.as_bytes())
            .map_err(|err| {
                InstrumentError::store(
                    "get_label_record",
                    CF_MEJEPA_Q4_REASONING_LABELS,
                    err.to_string(),
                    "read Q4 reasoning producer evidence by canonical row/session key",
                )
            })?
        else {
            return Ok(None);
        };
        serde_json::from_slice(&value).map(Some).map_err(|err| {
            InstrumentError::store(
                "deserialize_label_record",
                CF_MEJEPA_Q4_REASONING_LABELS,
                err.to_string(),
                "only mutate Q4 reasoning rows through Q4ReasoningLabelStore",
            )
        })
    }

    pub fn count_records(&self) -> InstrumentResult<usize> {
        Ok(self.scan_records()?.len())
    }

    pub fn count_calibrations(&self) -> InstrumentResult<usize> {
        Ok(self.scan_calibrations()?.len())
    }

    pub fn flush(&self) -> InstrumentResult<()> {
        self.db.flush_cf(self.signal_cf()?).map_err(|err| {
            InstrumentError::store(
                "flush",
                CF_MEJEPA_Q4_REASONING_LABELS,
                err.to_string(),
                "inspect RocksDB WAL and filesystem state",
            )
        })?;
        self.db.flush_cf(self.calibration_cf()?).map_err(|err| {
            InstrumentError::store(
                "flush",
                CF_MEJEPA_HEAD_CALIBRATIONS,
                err.to_string(),
                "inspect RocksDB WAL and filesystem state",
            )
        })
    }

    fn put_signal_record(&self, record: &PersistedQ4ReasoningSignal) -> InstrumentResult<String> {
        let value = serde_json::to_vec(record).map_err(|err| {
            InstrumentError::store(
                "serialize",
                CF_MEJEPA_Q4_REASONING_LABELS,
                err.to_string(),
                "ensure Q4 reasoning records remain JSON-serializable",
            )
        })?;
        let key = match &record.signal {
            Q4ReasoningSignalRecord::Label(label) => {
                q4_reasoning_label_key(&label.corpus_row_id, &label.session_id)
            }
            Q4ReasoningSignalRecord::Quarantine(quarantine) => format!(
                "quarantine::{}::{}::{}",
                quarantine.corpus_row_id, quarantine.session_id, quarantine.reason_code
            ),
        };
        self.put_sync_readback(
            CF_MEJEPA_Q4_REASONING_LABELS,
            self.signal_cf()?,
            &key,
            &value,
        )?;
        Ok(key)
    }

    fn put_calibration_record(
        &self,
        record: &PersistedQ4ReasoningCalibration,
    ) -> InstrumentResult<String> {
        let value = serde_json::to_vec(record).map_err(|err| {
            InstrumentError::store(
                "serialize",
                CF_MEJEPA_HEAD_CALIBRATIONS,
                err.to_string(),
                "ensure head calibration rows remain JSON-serializable",
            )
        })?;
        let key = q4_reasoning_calibration_key(record.row.class);
        self.put_sync_readback(
            CF_MEJEPA_HEAD_CALIBRATIONS,
            self.calibration_cf()?,
            &key,
            &value,
        )?;
        Ok(key)
    }

    fn put_sync_readback(
        &self,
        cf_name: &'static str,
        cf: &rocksdb::ColumnFamily,
        key: &str,
        value: &[u8],
    ) -> InstrumentResult<()> {
        let mut write_opts = WriteOptions::default();
        write_opts.set_sync(true);
        self.db
            .put_cf_opt(cf, key.as_bytes(), value, &write_opts)
            .map_err(|err| {
                InstrumentError::store(
                    "put_cf",
                    cf_name,
                    err.to_string(),
                    "inspect RocksDB write permissions, WAL state, and disk capacity",
                )
            })?;
        let readback = self.db.get_cf(cf, key.as_bytes()).map_err(|err| {
            InstrumentError::store(
                "get_cf",
                cf_name,
                err.to_string(),
                "inspect RocksDB read permissions and column-family health",
            )
        })?;
        if readback.as_deref() != Some(value) {
            return Err(InstrumentError::store(
                "read_after_write",
                cf_name,
                "row missing or changed after put_cf",
                "do not advance Q4 reasoning checkpoints until the CF row is readable",
            ));
        }
        Ok(())
    }

    fn signal_cf(&self) -> InstrumentResult<&rocksdb::ColumnFamily> {
        self.db
            .cf_handle(CF_MEJEPA_Q4_REASONING_LABELS)
            .ok_or_else(|| {
                InstrumentError::store(
                    "cf_handle",
                    CF_MEJEPA_Q4_REASONING_LABELS,
                    "column-family handle not found",
                    "open the store through Q4ReasoningLabelStore::open",
                )
            })
    }

    fn calibration_cf(&self) -> InstrumentResult<&rocksdb::ColumnFamily> {
        self.db
            .cf_handle(CF_MEJEPA_HEAD_CALIBRATIONS)
            .ok_or_else(|| {
                InstrumentError::store(
                    "cf_handle",
                    CF_MEJEPA_HEAD_CALIBRATIONS,
                    "column-family handle not found",
                    "open the store through Q4ReasoningLabelStore::open",
                )
            })
    }
}

fn decode_key(cf: &'static str, key: &[u8]) -> InstrumentResult<String> {
    String::from_utf8(key.to_vec()).map_err(|err| {
        InstrumentError::store(
            "decode_key",
            cf,
            err.to_string(),
            "Q4 reasoning/head-calibration keys must be UTF-8",
        )
    })
}
