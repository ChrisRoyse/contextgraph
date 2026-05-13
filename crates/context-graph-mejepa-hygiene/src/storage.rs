// Inspired by ruvnet/RuVector at HEAD ef5274c2 (clean-room reimplementation).

use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use dashmap::DashMap;
use fs2::FileExt;
use parking_lot::Mutex;
use rocksdb::{ColumnFamilyDescriptor, IteratorMode, Options, DB};
use serde::{de::DeserializeOwned, Serialize};

use crate::categories::{BYTES_PER_GB, DEFAULT_TOTAL_QUOTA_BYTES};
use crate::entry::{EntryId, HygieneEntryMeta};
use crate::error::{OpsError, OpsErrorKind, OpsResult};
use crate::retention::default_retention_policy_path;

pub trait SelfHealingHandle: Send + Sync {
    fn detect_drift(&self) -> OpsResult<serde_json::Value>;
    fn retrain_if_needed(&self) -> OpsResult<serde_json::Value>;
}

#[derive(Clone)]
pub struct RuntimeConfig {
    pub db: Arc<DB>,
    pub archive_root: PathBuf,
    pub total_quota_bytes: u64,
    pub witness_segment_size: usize,
    pub witness_min_age_days: u32,
    pub now_unix: Option<i64>,
    pub self_healing: Option<Arc<dyn SelfHealingHandle>>,
    pub retention_policy_path: Option<PathBuf>,
}

#[derive(Clone)]
pub struct HygieneEnv {
    pub config: RuntimeConfig,
    locks: Arc<DashMap<Vec<u8>, Arc<Mutex<()>>>>,
}

impl HygieneEnv {
    pub fn try_new(config: RuntimeConfig) -> OpsResult<Self> {
        if config.archive_root.as_os_str().is_empty() {
            return Err(OpsError::invalid(
                "archive_root",
                "archive_root must be non-empty",
            ));
        }
        if config.total_quota_bytes < BYTES_PER_GB {
            return Err(OpsError::invalid(
                "total_quota_bytes",
                "quota must be at least 1 GiB",
            ));
        }
        if config.witness_segment_size == 0 {
            return Err(OpsError::invalid(
                "witness_segment_size",
                "segment size must be >= 1",
            ));
        }
        for cf in context_graph_mejepa_cf::all_hygiene_referenced_cfs() {
            ensure_cf(&config.db, cf)?;
        }
        Ok(Self {
            config,
            locks: Arc::new(DashMap::new()),
        })
    }

    pub fn now_unix(&self) -> i64 {
        self.config
            .now_unix
            .unwrap_or_else(|| chrono::Utc::now().timestamp())
    }

    pub fn lock_for(&self, entry_id: &EntryId) -> Arc<Mutex<()>> {
        self.locks
            .entry(meta_key(entry_id))
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }
}

pub fn open_hygiene_rocksdb(path: impl AsRef<Path>) -> OpsResult<Arc<DB>> {
    let mut opts = Options::default();
    opts.create_if_missing(true);
    opts.create_missing_column_families(true);
    opts.set_paranoid_checks(true);
    let descriptors = context_graph_mejepa_cf::all_hygiene_referenced_cfs()
        .iter()
        .map(|cf| ColumnFamilyDescriptor::new(*cf, Options::default()))
        .collect::<Vec<_>>();
    Ok(Arc::new(DB::open_cf_descriptors(
        &opts,
        path.as_ref(),
        descriptors,
    )?))
}

pub fn runtime_config(db: Arc<DB>, archive_root: PathBuf) -> OpsResult<RuntimeConfig> {
    Ok(RuntimeConfig {
        db,
        archive_root,
        total_quota_bytes: quota_bytes_from_env()?,
        witness_segment_size: 1024,
        witness_min_age_days: 1,
        now_unix: None,
        self_healing: None,
        retention_policy_path: Some(default_retention_policy_path()),
    })
}

pub fn quota_bytes_from_env() -> OpsResult<u64> {
    match std::env::var("CG_MEJEPA_DISK_QUOTA_GB") {
        Ok(raw) => {
            let gb: u64 = raw.parse().map_err(|err| {
                OpsError::invalid(
                    "CG_MEJEPA_DISK_QUOTA_GB",
                    format!("expected integer GiB >= 1: {err}"),
                )
            })?;
            if gb == 0 {
                return Err(OpsError::invalid(
                    "CG_MEJEPA_DISK_QUOTA_GB",
                    "quota override must be >= 1 GiB",
                ));
            }
            Ok(gb.saturating_mul(BYTES_PER_GB))
        }
        Err(std::env::VarError::NotPresent) => Ok(DEFAULT_TOTAL_QUOTA_BYTES),
        Err(err) => Err(OpsError::invalid(
            "CG_MEJEPA_DISK_QUOTA_GB",
            format!("env var is not valid UTF-8: {err}"),
        )),
    }
}

pub fn cf<'a>(db: &'a DB, cf_name: &str) -> OpsResult<&'a rocksdb::ColumnFamily> {
    db.cf_handle(cf_name).ok_or_else(|| {
        OpsError::new(OpsErrorKind::MissingColumnFamily {
            cf_name: cf_name.to_string(),
        })
    })
}

pub fn ensure_cf(db: &DB, cf_name: &str) -> OpsResult<()> {
    cf(db, cf_name).map(|_| ())
}

pub fn meta_key(entry_id: &EntryId) -> Vec<u8> {
    let cf_bytes = entry_id.cf_name.as_bytes();
    let mut out = Vec::with_capacity(4 + cf_bytes.len() + entry_id.key.len());
    out.extend_from_slice(&(cf_bytes.len() as u32).to_be_bytes());
    out.extend_from_slice(cf_bytes);
    out.extend_from_slice(&entry_id.key);
    out
}

pub fn decode_meta_key(key: &[u8]) -> OpsResult<EntryId> {
    if key.len() < 4 {
        return Err(OpsError::new(OpsErrorKind::CorruptMetadata {
            key_hex: hex::encode(key),
            detail: "metadata key shorter than 4-byte CF length".to_string(),
        }));
    }
    let mut len = [0u8; 4];
    len.copy_from_slice(&key[..4]);
    let cf_len = u32::from_be_bytes(len) as usize;
    if key.len() < 4 + cf_len {
        return Err(OpsError::new(OpsErrorKind::CorruptMetadata {
            key_hex: hex::encode(key),
            detail: "metadata key truncated before CF name".to_string(),
        }));
    }
    let cf_name = std::str::from_utf8(&key[4..4 + cf_len])
        .map_err(|err| {
            OpsError::new(OpsErrorKind::CorruptMetadata {
                key_hex: hex::encode(key),
                detail: format!("CF name is not UTF-8: {err}"),
            })
        })?
        .to_string();
    if key.len() == 4 + cf_len {
        return Err(OpsError::new(OpsErrorKind::CorruptMetadata {
            key_hex: hex::encode(key),
            detail: "metadata key has empty row key".to_string(),
        }));
    }
    Ok(EntryId::new(cf_name, key[4 + cf_len..].to_vec()))
}

pub fn put_readback(db: &DB, cf_name: &str, key: &[u8], value: &[u8]) -> OpsResult<()> {
    let handle = cf(db, cf_name)?;
    db.put_cf(handle, key, value)?;
    let readback = db.get_cf(handle, key)?.ok_or_else(|| {
        OpsError::invalid(
            "rocksdb.readback",
            format!("missing row after put in {cf_name}"),
        )
    })?;
    if readback.as_slice() != value {
        return Err(OpsError::invalid(
            "rocksdb.readback",
            format!("readback mismatch in {cf_name}"),
        ));
    }
    Ok(())
}

pub fn read_meta(db: &DB, entry_id: &EntryId) -> OpsResult<Option<HygieneEntryMeta>> {
    let key = meta_key(entry_id);
    let Some(bytes) = db.get_cf(cf(db, context_graph_mejepa_cf::CF_MEJEPA_PANEL_META)?, &key)?
    else {
        return Ok(None);
    };
    let meta: HygieneEntryMeta = decode_cf_json(&bytes)?;
    meta.validate().map_err(|detail| {
        OpsError::new(OpsErrorKind::CorruptMetadata {
            key_hex: hex::encode(key),
            detail,
        })
    })?;
    Ok(Some(meta))
}

pub fn write_meta(db: &DB, meta: &HygieneEntryMeta) -> OpsResult<()> {
    meta.validate().map_err(|detail| {
        OpsError::new(OpsErrorKind::CorruptMetadata {
            key_hex: hex::encode(meta_key(&meta.entry_id)),
            detail,
        })
    })?;
    put_readback(
        db,
        context_graph_mejepa_cf::CF_MEJEPA_PANEL_META,
        &meta_key(&meta.entry_id),
        &encode_cf_json(meta)?,
    )
}

pub fn list_meta(db: &DB) -> OpsResult<Vec<HygieneEntryMeta>> {
    let mut out = Vec::new();
    for item in db.iterator_cf(
        cf(db, context_graph_mejepa_cf::CF_MEJEPA_PANEL_META)?,
        IteratorMode::Start,
    ) {
        let (key, value) = item?;
        let meta: HygieneEntryMeta = decode_cf_json(&value)?;
        meta.validate().map_err(|detail| {
            OpsError::new(OpsErrorKind::CorruptMetadata {
                key_hex: hex::encode(key),
                detail,
            })
        })?;
        out.push(meta);
    }
    Ok(out)
}

pub fn scan_cf(db: &DB, cf_name: &str) -> OpsResult<Vec<(Vec<u8>, Vec<u8>)>> {
    let mut rows = Vec::new();
    for item in db.iterator_cf(cf(db, cf_name)?, IteratorMode::Start) {
        let (key, value) = item?;
        rows.push((key.to_vec(), value.to_vec()));
    }
    Ok(rows)
}

pub fn count_and_bytes_cf(db: &DB, cf_name: &str) -> OpsResult<(u64, u64)> {
    let mut count = 0;
    let mut bytes = 0;
    for item in db.iterator_cf(cf(db, cf_name)?, IteratorMode::Start) {
        let (_key, value) = item?;
        count += 1;
        bytes += value.len() as u64;
    }
    Ok((count, bytes))
}

pub fn encode_cf_json<T: Serialize>(value: &T) -> OpsResult<Vec<u8>> {
    Ok(serde_json::to_vec(value)?)
}

pub fn decode_cf_json<T: DeserializeOwned>(bytes: &[u8]) -> OpsResult<T> {
    Ok(serde_json::from_slice(bytes)?)
}

pub fn open_exclusive_lock(path: &Path) -> OpsResult<std::fs::File> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| OpsError::io("create_dir_all", parent, err))?;
    }
    let mut options = OpenOptions::new();
    options.create(true).truncate(false).read(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let lock = options
        .open(path)
        .map_err(|err| OpsError::io("open", path, err))?;
    lock.try_lock_exclusive().map_err(|err| {
        OpsError::new(OpsErrorKind::Lock {
            path: path.to_path_buf(),
            detail: err.to_string(),
        })
    })?;
    Ok(lock)
}

pub fn operation_lock_path(archive_root: &Path, operation: &str) -> PathBuf {
    archive_root
        .join(".locks")
        .join(format!("{operation}.lock"))
}
