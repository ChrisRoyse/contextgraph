use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use bincode::Options as BincodeOptions;
use rocksdb::{
    ColumnFamily, ColumnFamilyDescriptor, FlushOptions, IteratorMode, Options, WriteBatch,
    WriteOptions, DB,
};

use crate::constellation::{bincode_options, TctConstellation};
use crate::error::TctError;
use crate::freshness::read_freshness_config;
use crate::freshness_policy::ConstellationRefreshLogEntry;
use crate::rate_aggregator::CF_MEJEPA_GUARD_DECISIONS;
use crate::refresh_report::ConstellationRefreshReport;
use crate::types::EmbedderId;

pub use context_graph_mejepa_cf::{
    CF_MEJEPA_CONSTELLATION, CF_MEJEPA_CONSTELLATION_REFRESH_LOG,
    CF_MEJEPA_CONSTELLATION_REFRESH_REPORTS, TCT_CFS as MEJEPA_TCT_CFS,
};

#[derive(Clone)]
pub struct ConstellationStore {
    db: Arc<DB>,
}

impl ConstellationStore {
    pub fn new(db: Arc<DB>) -> Result<Self, TctError> {
        for cf_name in MEJEPA_TCT_CFS {
            if db.cf_handle(cf_name).is_none() {
                return Err(TctError::store("open", cf_name, "column family missing"));
            }
        }
        Ok(Self { db })
    }

    pub fn db(&self) -> Arc<DB> {
        self.db.clone()
    }

    pub fn persist(&self, constellation: &TctConstellation) -> Result<[u8; 32], TctError> {
        constellation.validate_integrity()?;
        let key = constellation_key(
            constellation.version_id(),
            constellation.corpus_provenance.corpus_sha,
        );
        let bytes = bincode_options().serialize(constellation)?;
        let mut batch = WriteBatch::default();
        batch.put_cf(cf(&self.db, CF_MEJEPA_CONSTELLATION)?, &key, bytes);
        self.db
            .write_opt(batch, &durable_write_options())
            .map_err(|err| {
                TctError::store("write_batch", CF_MEJEPA_CONSTELLATION, err.to_string())
            })?;
        durable_flush_cf(&self.db, CF_MEJEPA_CONSTELLATION)?;
        let raw = self
            .db
            .get_cf(cf(&self.db, CF_MEJEPA_CONSTELLATION)?, &key)
            .map_err(|err| TctError::store("get", CF_MEJEPA_CONSTELLATION, err.to_string()))?
            .ok_or_else(|| {
                TctError::store(
                    "read_after_write",
                    CF_MEJEPA_CONSTELLATION,
                    "missing persisted row",
                )
            })?;
        let readback: TctConstellation = bincode_options().deserialize(&raw)?;
        readback.validate_integrity()?;
        if readback.version_id() != constellation.version_id() {
            return Err(TctError::FrozenViolation {
                detail: "read-after-write version mismatch".to_string(),
            });
        }
        Ok(constellation.version_id())
    }

    pub fn load(
        &self,
        version_id: [u8; 32],
        runtime_embedder_versions: &BTreeMap<EmbedderId, [u8; 32]>,
    ) -> Result<TctConstellation, TctError> {
        let constellation = self.load_without_runtime_checks(version_id)?;
        constellation.check_provenance(runtime_embedder_versions)?;
        let (max_age_days, allow_stale) = read_freshness_config()?;
        constellation.check_freshness(max_age_days, allow_stale)?;
        Ok(constellation)
    }

    pub fn load_without_runtime_checks(
        &self,
        version_id: [u8; 32],
    ) -> Result<TctConstellation, TctError> {
        let matches = self.raw_values_for_version(version_id)?;
        if matches.is_empty() {
            return Err(TctError::MissingCentroid {
                detail: format!(
                    "no constellation row for version {}",
                    hex::encode(version_id)
                ),
            });
        }
        if matches.len() > 1 {
            return Err(TctError::Store {
                operation: "load",
                cf: CF_MEJEPA_CONSTELLATION,
                detail: format!(
                    "ambiguous version {} has {} corpus rows",
                    hex::encode(version_id),
                    matches.len()
                ),
            });
        }
        let constellation: TctConstellation = bincode_options().deserialize(&matches[0])?;
        constellation.validate_integrity()?;
        if constellation.version_id() != version_id {
            return Err(TctError::FrozenViolation {
                detail: "key version_id does not match payload".to_string(),
            });
        }
        Ok(constellation)
    }

    pub fn list_versions(&self, limit: usize) -> Result<Vec<[u8; 32]>, TctError> {
        if limit == 0 {
            return Err(TctError::invalid(
                "limit",
                "list_versions limit must be positive",
            ));
        }
        let cf = cf(&self.db, CF_MEJEPA_CONSTELLATION)?;
        let mut versions = Vec::new();
        for item in self.db.iterator_cf(cf, IteratorMode::Start) {
            let (key, value) = item.map_err(|err| {
                TctError::store("iterate", CF_MEJEPA_CONSTELLATION, err.to_string())
            })?;
            if key.len() != 64 {
                return Err(TctError::dim(
                    64,
                    key.len(),
                    "constellation key must be version_id || corpus_sha",
                ));
            }
            let mut version = [0u8; 32];
            version.copy_from_slice(&key[..32]);
            let constellation: TctConstellation = bincode_options().deserialize(&value)?;
            constellation.validate_integrity()?;
            if constellation.version_id() != version {
                return Err(TctError::FrozenViolation {
                    detail: "constellation key version does not match payload".to_string(),
                });
            }
            if !versions.contains(&version) {
                versions.push(version);
            }
            if versions.len() == limit {
                break;
            }
        }
        Ok(versions)
    }

    pub fn latest_version(&self) -> Result<[u8; 32], TctError> {
        let cf = cf(&self.db, CF_MEJEPA_CONSTELLATION)?;
        let mut latest: Option<TctConstellation> = None;
        for item in self.db.iterator_cf(cf, IteratorMode::Start) {
            let (_key, value) = item.map_err(|err| {
                TctError::store("iterate", CF_MEJEPA_CONSTELLATION, err.to_string())
            })?;
            let constellation: TctConstellation = bincode_options().deserialize(&value)?;
            constellation.validate_integrity()?;
            let replace = latest
                .as_ref()
                .map(|current| {
                    constellation.frozen_at > current.frozen_at
                        || (constellation.frozen_at == current.frozen_at
                            && constellation.version_id() > current.version_id())
                })
                .unwrap_or(true);
            if replace {
                latest = Some(constellation);
            }
        }
        latest
            .map(|constellation| constellation.version_id())
            .ok_or_else(|| TctError::MissingCentroid {
                detail: "no persisted TCT constellations".to_string(),
            })
    }

    pub fn count_constellations(&self) -> Result<usize, TctError> {
        count_cf(&self.db, CF_MEJEPA_CONSTELLATION)
    }

    pub fn count_guard_decisions(&self) -> Result<usize, TctError> {
        count_cf(&self.db, CF_MEJEPA_GUARD_DECISIONS)
    }

    pub fn persist_refresh_report(
        &self,
        report: &ConstellationRefreshReport,
    ) -> Result<[u8; 32], TctError> {
        report.validate_integrity()?;
        let key = refresh_report_key(report.finished_at, report.report_id)?;
        let bytes = bincode_options().serialize(report)?;
        self.db
            .put_cf_opt(
                cf(&self.db, CF_MEJEPA_CONSTELLATION_REFRESH_REPORTS)?,
                &key,
                bytes,
                &durable_write_options(),
            )
            .map_err(|err| {
                TctError::store(
                    "put",
                    CF_MEJEPA_CONSTELLATION_REFRESH_REPORTS,
                    err.to_string(),
                )
            })?;
        durable_flush_cf(&self.db, CF_MEJEPA_CONSTELLATION_REFRESH_REPORTS)?;
        let raw = self
            .db
            .get_cf(cf(&self.db, CF_MEJEPA_CONSTELLATION_REFRESH_REPORTS)?, &key)
            .map_err(|err| {
                TctError::store(
                    "get",
                    CF_MEJEPA_CONSTELLATION_REFRESH_REPORTS,
                    err.to_string(),
                )
            })?
            .ok_or_else(|| {
                TctError::store(
                    "read_after_write",
                    CF_MEJEPA_CONSTELLATION_REFRESH_REPORTS,
                    "missing persisted refresh report row",
                )
            })?;
        let readback: ConstellationRefreshReport = bincode_options().deserialize(&raw)?;
        readback.validate_integrity()?;
        if readback != *report {
            return Err(TctError::FrozenViolation {
                detail: "refresh report read-after-write payload mismatch".to_string(),
            });
        }
        Ok(report.report_id)
    }

    pub fn load_refresh_report(
        &self,
        report_id: [u8; 32],
    ) -> Result<ConstellationRefreshReport, TctError> {
        let mut matches = self.refresh_report_values_by_id(report_id)?;
        if matches.is_empty() {
            return Err(TctError::MissingCentroid {
                detail: format!(
                    "no refresh report row for report {}",
                    hex::encode(report_id)
                ),
            });
        }
        if matches.len() > 1 {
            return Err(TctError::Store {
                operation: "load_refresh_report",
                cf: CF_MEJEPA_CONSTELLATION_REFRESH_REPORTS,
                detail: format!(
                    "ambiguous refresh report {} has {} rows",
                    hex::encode(report_id),
                    matches.len()
                ),
            });
        }
        let report = matches.remove(0);
        report.validate_integrity()?;
        Ok(report)
    }

    pub fn latest_refresh_report(&self) -> Result<ConstellationRefreshReport, TctError> {
        let cf = cf(&self.db, CF_MEJEPA_CONSTELLATION_REFRESH_REPORTS)?;
        let mut latest: Option<ConstellationRefreshReport> = None;
        for item in self.db.iterator_cf(cf, IteratorMode::Start) {
            let (_key, value) = item.map_err(|err| {
                TctError::store(
                    "iterate",
                    CF_MEJEPA_CONSTELLATION_REFRESH_REPORTS,
                    err.to_string(),
                )
            })?;
            let report: ConstellationRefreshReport = bincode_options().deserialize(&value)?;
            report.validate_integrity()?;
            let replace = latest
                .as_ref()
                .map(|current| {
                    report.finished_at > current.finished_at
                        || (report.finished_at == current.finished_at
                            && report.report_id > current.report_id)
                })
                .unwrap_or(true);
            if replace {
                latest = Some(report);
            }
        }
        latest.ok_or_else(|| TctError::MissingCentroid {
            detail: "no persisted TCT refresh reports".to_string(),
        })
    }

    pub fn refresh_reports_for_constellation(
        &self,
        version_id: [u8; 32],
        limit: usize,
    ) -> Result<Vec<ConstellationRefreshReport>, TctError> {
        if limit == 0 {
            return Err(TctError::invalid(
                "limit",
                "refresh report limit must be positive",
            ));
        }
        let cf = cf(&self.db, CF_MEJEPA_CONSTELLATION_REFRESH_REPORTS)?;
        let mut reports = Vec::new();
        for item in self.db.iterator_cf(cf, IteratorMode::Start) {
            let (_key, value) = item.map_err(|err| {
                TctError::store(
                    "iterate",
                    CF_MEJEPA_CONSTELLATION_REFRESH_REPORTS,
                    err.to_string(),
                )
            })?;
            let report: ConstellationRefreshReport = bincode_options().deserialize(&value)?;
            report.validate_integrity()?;
            if report.constellation_version_id == version_id {
                reports.push(report);
            }
        }
        reports.sort_by(|left, right| {
            right
                .finished_at
                .cmp(&left.finished_at)
                .then_with(|| right.report_id.cmp(&left.report_id))
        });
        reports.truncate(limit);
        Ok(reports)
    }

    pub fn count_refresh_reports(&self) -> Result<usize, TctError> {
        count_cf(&self.db, CF_MEJEPA_CONSTELLATION_REFRESH_REPORTS)
    }

    pub fn persist_refresh_log_entry(
        &self,
        entry: &ConstellationRefreshLogEntry,
    ) -> Result<[u8; 32], TctError> {
        entry.validate_integrity()?;
        let key = refresh_log_key(entry.generated_at, entry.event_id)?;
        let bytes = bincode_options().serialize(entry)?;
        self.db
            .put_cf_opt(
                cf(&self.db, CF_MEJEPA_CONSTELLATION_REFRESH_LOG)?,
                &key,
                bytes,
                &durable_write_options(),
            )
            .map_err(|err| {
                TctError::store("put", CF_MEJEPA_CONSTELLATION_REFRESH_LOG, err.to_string())
            })?;
        durable_flush_cf(&self.db, CF_MEJEPA_CONSTELLATION_REFRESH_LOG)?;
        let raw = self
            .db
            .get_cf(cf(&self.db, CF_MEJEPA_CONSTELLATION_REFRESH_LOG)?, &key)
            .map_err(|err| {
                TctError::store("get", CF_MEJEPA_CONSTELLATION_REFRESH_LOG, err.to_string())
            })?
            .ok_or_else(|| {
                TctError::store(
                    "read_after_write",
                    CF_MEJEPA_CONSTELLATION_REFRESH_LOG,
                    "missing persisted refresh log row",
                )
            })?;
        let readback: ConstellationRefreshLogEntry = bincode_options().deserialize(&raw)?;
        readback.validate_integrity()?;
        if readback != *entry {
            return Err(TctError::FrozenViolation {
                detail: "refresh log read-after-write payload mismatch".to_string(),
            });
        }
        Ok(entry.event_id)
    }

    pub fn load_refresh_log_entries(
        &self,
        limit: usize,
    ) -> Result<Vec<ConstellationRefreshLogEntry>, TctError> {
        if limit == 0 {
            return Err(TctError::invalid(
                "limit",
                "refresh log limit must be positive",
            ));
        }
        let cf = cf(&self.db, CF_MEJEPA_CONSTELLATION_REFRESH_LOG)?;
        let mut entries = Vec::new();
        for item in self.db.iterator_cf(cf, IteratorMode::End) {
            let (key, value) = item.map_err(|err| {
                TctError::store(
                    "iterate",
                    CF_MEJEPA_CONSTELLATION_REFRESH_LOG,
                    err.to_string(),
                )
            })?;
            if key.len() != 40 {
                return Err(TctError::dim(
                    40,
                    key.len(),
                    "refresh log key must be generated_at_secs || event_id",
                ));
            }
            let entry: ConstellationRefreshLogEntry = bincode_options().deserialize(&value)?;
            entry.validate_integrity()?;
            entries.push(entry);
            if entries.len() == limit {
                break;
            }
        }
        entries.reverse();
        Ok(entries)
    }

    pub fn count_refresh_log_entries(&self) -> Result<usize, TctError> {
        count_cf(&self.db, CF_MEJEPA_CONSTELLATION_REFRESH_LOG)
    }

    pub fn read_raw_by_version(&self, version_id: [u8; 32]) -> Result<Vec<Vec<u8>>, TctError> {
        self.raw_values_for_version(version_id)
    }

    fn refresh_report_values_by_id(
        &self,
        report_id: [u8; 32],
    ) -> Result<Vec<ConstellationRefreshReport>, TctError> {
        let cf = cf(&self.db, CF_MEJEPA_CONSTELLATION_REFRESH_REPORTS)?;
        let mut out = Vec::new();
        for item in self.db.iterator_cf(cf, IteratorMode::Start) {
            let (key, value) = item.map_err(|err| {
                TctError::store(
                    "iterate",
                    CF_MEJEPA_CONSTELLATION_REFRESH_REPORTS,
                    err.to_string(),
                )
            })?;
            if key.len() != 40 {
                return Err(TctError::dim(
                    40,
                    key.len(),
                    "refresh report key must be finished_at_secs || report_id",
                ));
            }
            if key[8..] == report_id {
                let report: ConstellationRefreshReport = bincode_options().deserialize(&value)?;
                report.validate_integrity()?;
                out.push(report);
            }
        }
        Ok(out)
    }

    fn raw_values_for_version(&self, version_id: [u8; 32]) -> Result<Vec<Vec<u8>>, TctError> {
        let cf = cf(&self.db, CF_MEJEPA_CONSTELLATION)?;
        let mut out = Vec::new();
        for item in self.db.iterator_cf(cf, IteratorMode::Start) {
            let (key, value) = item.map_err(|err| {
                TctError::store("iterate", CF_MEJEPA_CONSTELLATION, err.to_string())
            })?;
            if key.len() != 64 {
                return Err(TctError::dim(
                    64,
                    key.len(),
                    "constellation key must be version_id || corpus_sha",
                ));
            }
            if key.starts_with(&version_id) {
                out.push(value.to_vec());
            }
        }
        Ok(out)
    }
}

pub fn open_tct_rocksdb(path: impl AsRef<Path>) -> Result<Arc<DB>, TctError> {
    let mut db_opts = Options::default();
    db_opts.create_if_missing(true);
    db_opts.create_missing_column_families(true);
    db_opts.set_paranoid_checks(true);
    let descriptors = MEJEPA_TCT_CFS
        .iter()
        .map(|name| ColumnFamilyDescriptor::new(*name, cf_options()))
        .collect::<Vec<_>>();
    let db = DB::open_cf_descriptors(&db_opts, path.as_ref(), descriptors)
        .map_err(|err| TctError::store("open", "<all>", err.to_string()))?;
    for cf_name in MEJEPA_TCT_CFS {
        if db.cf_handle(cf_name).is_none() {
            return Err(TctError::store(
                "open",
                cf_name,
                "column family missing after open",
            ));
        }
    }
    Ok(Arc::new(db))
}

pub fn cf<'a>(db: &'a DB, cf_name: &'static str) -> Result<&'a ColumnFamily, TctError> {
    db.cf_handle(cf_name)
        .ok_or_else(|| TctError::store("cf_handle", cf_name, "column family missing"))
}

pub fn count_cf(db: &DB, cf_name: &'static str) -> Result<usize, TctError> {
    let cf = cf(db, cf_name)?;
    let mut count = 0usize;
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        item.map_err(|err| TctError::store("iterate", cf_name, err.to_string()))?;
        count += 1;
    }
    Ok(count)
}

fn cf_options() -> Options {
    let mut opts = Options::default();
    opts.set_paranoid_checks(true);
    opts
}

fn durable_write_options() -> WriteOptions {
    let mut opts = WriteOptions::default();
    opts.set_sync(true);
    opts.disable_wal(false);
    opts
}

fn durable_flush_cf(db: &DB, cf_name: &'static str) -> Result<(), TctError> {
    db.flush_wal(true)
        .map_err(|err| TctError::store("flush_wal", cf_name, err.to_string()))?;
    let mut flush_opts = FlushOptions::default();
    flush_opts.set_wait(true);
    db.flush_cf_opt(cf(db, cf_name)?, &flush_opts)
        .map_err(|err| TctError::store("flush_cf", cf_name, err.to_string()))?;
    Ok(())
}

fn constellation_key(version_id: [u8; 32], corpus_sha: [u8; 32]) -> Vec<u8> {
    let mut key = Vec::with_capacity(64);
    key.extend_from_slice(&version_id);
    key.extend_from_slice(&corpus_sha);
    key
}

fn refresh_report_key(finished_at: SystemTime, report_id: [u8; 32]) -> Result<Vec<u8>, TctError> {
    let secs = finished_at
        .duration_since(UNIX_EPOCH)
        .map_err(|_| {
            TctError::invalid(
                "refresh_report.finished_at",
                "finished_at must not predate UNIX_EPOCH",
            )
        })?
        .as_secs();
    let mut key = Vec::with_capacity(40);
    key.extend_from_slice(&secs.to_be_bytes());
    key.extend_from_slice(&report_id);
    Ok(key)
}

fn refresh_log_key(generated_at: SystemTime, event_id: [u8; 32]) -> Result<Vec<u8>, TctError> {
    let secs = generated_at
        .duration_since(UNIX_EPOCH)
        .map_err(|_| {
            TctError::invalid(
                "refresh_log.generated_at",
                "generated_at must not predate UNIX_EPOCH",
            )
        })?
        .as_secs();
    let mut key = Vec::with_capacity(40);
    key.extend_from_slice(&secs.to_be_bytes());
    key.extend_from_slice(&event_id);
    Ok(key)
}
