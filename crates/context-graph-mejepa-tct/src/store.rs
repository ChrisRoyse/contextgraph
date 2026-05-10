use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use bincode::Options as BincodeOptions;
use rocksdb::{ColumnFamily, ColumnFamilyDescriptor, IteratorMode, Options, WriteBatch, DB};

use crate::constellation::{bincode_options, TctConstellation};
use crate::error::TctError;
use crate::freshness::read_freshness_config;
use crate::rate_aggregator::CF_MEJEPA_GUARD_DECISIONS;
use crate::types::EmbedderId;

pub const CF_MEJEPA_CONSTELLATION: &str = "CF_MEJEPA_CONSTELLATION";
pub const MEJEPA_TCT_CFS: &[&str] = &[CF_MEJEPA_CONSTELLATION, CF_MEJEPA_GUARD_DECISIONS];

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
        self.db.write(batch).map_err(|err| {
            TctError::store("write_batch", CF_MEJEPA_CONSTELLATION, err.to_string())
        })?;
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

    pub fn read_raw_by_version(&self, version_id: [u8; 32]) -> Result<Vec<Vec<u8>>, TctError> {
        self.raw_values_for_version(version_id)
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

fn constellation_key(version_id: [u8; 32], corpus_sha: [u8; 32]) -> Vec<u8> {
    let mut key = Vec::with_capacity(64);
    key.extend_from_slice(&version_id);
    key.extend_from_slice(&corpus_sha);
    key
}
