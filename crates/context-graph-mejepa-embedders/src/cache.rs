use crate::cache_entry::{CacheEntryRecord, CachedEmbedderVector};
use crate::cache_io::{
    cache_json_parse_error, checked_inc, fsync_dir, is_lower_hex_sha256, unix_ms,
    visit_cache_files, write_bytes_atomic,
};
use crate::cache_limits::{
    EmbedderCacheConfig, EmbedderCacheTelemetry, PruneReport, EMBEDDER_CACHE_TELEMETRY_FILE,
};
use crate::embedder_id::EmbedderId;
use crate::error::{EmbedError, EmbedResult};
use crate::forward::EmbedderForward;
use crate::types::{EmbedderInput, EmbedderOutput};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub use crate::cache_entry::chunk_sha256;
pub const EMBEDDER_CACHE_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmbedderCacheKey {
    pub schema_version: u16,
    pub embedder: EmbedderId,
    pub model_version: String,
    pub chunk_sha256: String,
}

impl EmbedderCacheKey {
    pub fn new(
        embedder: EmbedderId,
        model_version: impl Into<String>,
        chunk_sha256: String,
    ) -> Self {
        Self {
            schema_version: EMBEDDER_CACHE_SCHEMA_VERSION,
            embedder,
            model_version: model_version.into(),
            chunk_sha256,
        }
    }

    pub fn for_input(forwarder: &dyn EmbedderForward, input: &EmbedderInput) -> EmbedResult<Self> {
        input.validate()?;
        if forwarder.embedder() != input.embedder {
            return Err(EmbedError::forward(
                forwarder.embedder(),
                format!(
                    "cache key embedder mismatch: forwarder={} input={}",
                    forwarder.embedder(),
                    input.embedder
                ),
                "route the input to the matching embedder before cache lookup",
            ));
        }
        let key = Self::new(
            input.embedder,
            forwarder.model_version().to_string(),
            chunk_sha256(&input.text),
        );
        key.validate()?;
        Ok(key)
    }

    pub fn validate(&self) -> EmbedResult<()> {
        if self.schema_version != EMBEDDER_CACHE_SCHEMA_VERSION {
            return Err(EmbedError::invalid(
                "EmbedderCacheKey.schema_version",
                format!(
                    "cache key schema_version {} does not match runtime schema {}",
                    self.schema_version, EMBEDDER_CACHE_SCHEMA_VERSION
                ),
                "clear or migrate the embedder cache before using this runtime",
            ));
        }
        if self.model_version.trim().is_empty() {
            return Err(EmbedError::invalid(
                "EmbedderCacheKey.model_version",
                "model_version is empty",
                "include the SHA-pinned model manifest or algorithm version in every cache key",
            ));
        }
        if !is_lower_hex_sha256(&self.chunk_sha256) {
            return Err(EmbedError::invalid(
                "EmbedderCacheKey.chunk_sha256",
                "chunk_sha256 must be 64 lowercase hex characters",
                "hash the exact chunk bytes with SHA-256 before cache lookup",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct EmbedderCache {
    root: PathBuf,
    config: EmbedderCacheConfig,
}

impl EmbedderCache {
    pub fn new(root: impl Into<PathBuf>) -> EmbedResult<Self> {
        Self::new_with_config(root, EmbedderCacheConfig::from_env()?)
    }
    pub fn new_with_config(
        root: impl Into<PathBuf>,
        config: EmbedderCacheConfig,
    ) -> EmbedResult<Self> {
        let root = root.into();
        if root.as_os_str().is_empty() {
            return Err(EmbedError::invalid(
                "EmbedderCache.root",
                "cache root path is empty",
                "configure a concrete local cache directory on durable storage",
            ));
        }
        config.validate()?;
        Ok(Self { root, config })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn config(&self) -> EmbedderCacheConfig {
        self.config
    }
    pub fn telemetry_path(&self) -> PathBuf {
        self.root.join(EMBEDDER_CACHE_TELEMETRY_FILE)
    }
    pub fn telemetry(&self) -> EmbedResult<EmbedderCacheTelemetry> {
        self.load_telemetry()
    }

    pub fn get(
        &self,
        key: &EmbedderCacheKey,
        source_id: String,
    ) -> EmbedResult<Option<EmbedderOutput>> {
        key.validate()?;
        let path = self.entry_path(key);
        if !path.exists() {
            self.record_miss()?;
            return Ok(None);
        }
        let bytes = fs::read(&path).map_err(|err| {
            EmbedError::forward(
                key.embedder,
                format!("cache read failed at {}: {err}", path.display()),
                "inspect cache filesystem permissions and storage health; corrupt cache reads are not treated as misses",
            )
        })?;
        let mut cached: CachedEmbedderVector = serde_json::from_slice(&bytes).map_err(|err| {
            let _ = self.record_corrupt_entry_error();
            cache_json_parse_error(key.embedder, &path, err)
        })?;
        if let Err(err) = cached.validate() {
            self.record_corrupt_entry_error()?;
            return Err(err);
        }
        if cached.key != *key {
            self.record_corrupt_entry_error()?;
            return Err(EmbedError::forward(
                key.embedder,
                format!(
                    "cache key mismatch at {}: requested {:?}, stored {:?}",
                    path.display(),
                    key,
                    cached.key
                ),
                "delete the corrupt cache entry; cache files must be content-addressed by the stored key",
            ));
        }
        cached.touch()?;
        self.write_cached_entry(&path, &cached)?;
        self.record_hit()?;
        cached.into_output(source_id).map(Some)
    }

    pub fn put_and_verify(
        &self,
        key: &EmbedderCacheKey,
        output: &EmbedderOutput,
    ) -> EmbedResult<()> {
        key.validate()?;
        let cached = CachedEmbedderVector::from_output(key.clone(), output)?;
        let path = self.entry_path(key);
        let bytes = serde_json::to_vec(&cached).map_err(|err| {
            EmbedError::forward(
                key.embedder,
                format!("cache JSON serialization failed: {err}"),
                "ensure cached vector metadata remains JSON serializable",
            )
        })?;
        if bytes.len() as u64 > self.config.max_bytes {
            self.record_write_rejection()?;
            return Err(EmbedError::forward(
                key.embedder,
                format!(
                    "cache entry {} is {} bytes, larger than configured cache max_bytes {}",
                    path.display(),
                    bytes.len(),
                    self.config.max_bytes
                ),
                "raise CG_MEJEPA_EMBED_CACHE_MAX_BYTES or reduce the embedder output size before caching",
            ));
        }
        write_bytes_atomic(&path, &bytes, key.embedder)?;
        let readback_bytes = fs::read(&path).map_err(|err| {
            EmbedError::forward(
                key.embedder,
                format!(
                    "cache readback failed immediately after write at {}: {err}",
                    path.display()
                ),
                "inspect cache filesystem permissions and storage health",
            )
        })?;
        if readback_bytes != bytes {
            return Err(EmbedError::forward(
                key.embedder,
                "cache readback bytes did not match the just-written payload",
                "inspect disk write ordering and filesystem integrity",
            ));
        }
        let readback: CachedEmbedderVector =
            serde_json::from_slice(&readback_bytes).map_err(|err| {
                EmbedError::forward(
                    key.embedder,
                    format!(
                        "cache readback JSON parse failed at {}: {err}",
                        path.display()
                    ),
                    "delete the corrupt cache entry and inspect disk write ordering",
                )
            })?;
        readback.validate()?;
        if readback.vector != output.vector
            || readback.key.model_version != output.model_version
            || readback.precision_class != output.precision_class
        {
            return Err(EmbedError::forward(
                key.embedder,
                "cache readback did not match the written embedder output",
                "delete the corrupt cache entry and inspect disk write ordering",
            ));
        }
        self.record_write()?;
        self.enforce_limits(key.embedder)?;
        Ok(())
    }

    pub async fn forward_cached(
        &self,
        forwarder: &dyn EmbedderForward,
        input: &EmbedderInput,
    ) -> EmbedResult<EmbedderOutput> {
        let key = EmbedderCacheKey::for_input(forwarder, input)?;
        if let Some(output) = self.get(&key, input.source_id.clone())? {
            return Ok(output);
        }
        let output = forwarder.forward(input).await?;
        self.put_and_verify(&key, &output)?;
        Ok(output)
    }

    pub fn entry_path(&self, key: &EmbedderCacheKey) -> PathBuf {
        self.root
            .join(format!("{:?}", key.embedder))
            .join(chunk_sha256(&key.model_version))
            .join(format!("{}.json", key.chunk_sha256))
    }

    pub fn enforce_limits(&self, embedder: EmbedderId) -> EmbedResult<PruneReport> {
        let mut entries = self.entry_records(embedder)?;
        let mut total_bytes = entries.iter().map(|entry| entry.byte_count).sum::<u64>();
        let mut report = PruneReport {
            before_entries: entries.len(),
            before_bytes: total_bytes,
            ..PruneReport::default()
        };
        entries.sort_by(|left, right| {
            left.last_accessed_unix_ms
                .cmp(&right.last_accessed_unix_ms)
                .then_with(|| left.path.cmp(&right.path))
        });
        while entries.len().saturating_sub(report.evicted_entries) > self.config.max_entries
            || total_bytes > self.config.max_bytes
        {
            let Some(victim) = entries.get(report.evicted_entries) else {
                return Err(EmbedError::forward(
                    embedder,
                    "cache limit enforcement ran out of eviction candidates",
                    "inspect cache inventory and configured bounds",
                ));
            };
            fs::remove_file(&victim.path).map_err(|err| {
                EmbedError::forward(
                    embedder,
                    format!("cache eviction failed at {}: {err}", victim.path.display()),
                    "inspect cache filesystem permissions; eviction failures are not ignored",
                )
            })?;
            if let Some(parent) = victim.path.parent() {
                fsync_dir(parent, embedder)?;
            }
            total_bytes = total_bytes.saturating_sub(victim.byte_count);
            report.evicted_entries += 1;
            report.evicted_bytes = report.evicted_bytes.saturating_add(victim.byte_count);
            report.evicted_paths.push(victim.path.display().to_string());
        }
        report.after_entries = entries.len().saturating_sub(report.evicted_entries);
        report.after_bytes = total_bytes;
        if report.evicted_entries > 0 {
            self.record_eviction(report.evicted_entries as u64, report.evicted_bytes)?;
        } else {
            self.refresh_telemetry_inventory()?;
        }
        Ok(report)
    }

    fn write_cached_entry(&self, path: &Path, cached: &CachedEmbedderVector) -> EmbedResult<()> {
        let bytes = serde_json::to_vec(cached).map_err(|err| {
            EmbedError::forward(
                cached.key.embedder,
                format!("cache JSON serialization failed: {err}"),
                "ensure cached vector metadata remains JSON serializable",
            )
        })?;
        write_bytes_atomic(path, &bytes, cached.key.embedder)
    }
    fn entry_records(&self, embedder: EmbedderId) -> EmbedResult<Vec<CacheEntryRecord>> {
        let mut out = Vec::new();
        if !self.root.exists() {
            return Ok(out);
        }
        let telemetry_path = self.telemetry_path();
        visit_cache_files(&self.root, &mut |path| {
            if path == telemetry_path
                || path.extension().and_then(|ext| ext.to_str()) != Some("json")
            {
                return Ok(());
            }
            let bytes = fs::read(path).map_err(|err| {
                EmbedError::forward(
                    embedder,
                    format!("cache inventory read failed at {}: {err}", path.display()),
                    "inspect cache filesystem permissions before pruning",
                )
            })?;
            let cached: CachedEmbedderVector =
                serde_json::from_slice(&bytes).map_err(|err| {
                    EmbedError::forward(
                    embedder,
                    format!("cache inventory JSON parse failed at {}: {err}", path.display()),
                    "delete the corrupt cache entry; pruning refuses to guess around corrupt rows",
                )
                })?;
            cached.validate()?;
            out.push(CacheEntryRecord {
                path: path.to_path_buf(),
                byte_count: fs::metadata(path)
                    .map_err(|err| {
                        EmbedError::forward(
                            embedder,
                            format!(
                                "cache inventory metadata failed at {}: {err}",
                                path.display()
                            ),
                            "inspect cache filesystem permissions before pruning",
                        )
                    })?
                    .len(),
                last_accessed_unix_ms: cached.last_accessed_unix_ms,
            });
            Ok(())
        })?;
        Ok(out)
    }
    fn load_telemetry(&self) -> EmbedResult<EmbedderCacheTelemetry> {
        let path = self.telemetry_path();
        if !path.exists() {
            return Ok(EmbedderCacheTelemetry::new(
                EMBEDDER_CACHE_SCHEMA_VERSION,
                self.config,
                unix_ms()?,
            ));
        }
        let bytes = fs::read(&path).map_err(|err| {
            EmbedError::invalid(
                "EmbedderCacheTelemetry",
                format!("telemetry read failed at {}: {err}", path.display()),
                "inspect cache telemetry filesystem permissions",
            )
        })?;
        let telemetry: EmbedderCacheTelemetry = serde_json::from_slice(&bytes).map_err(|err| {
            EmbedError::invalid(
                "EmbedderCacheTelemetry",
                format!("telemetry JSON parse failed at {}: {err}", path.display()),
                "delete or repair the corrupt telemetry file before continuing",
            )
        })?;
        telemetry.validate(EMBEDDER_CACHE_SCHEMA_VERSION, self.config)?;
        Ok(telemetry)
    }
    fn write_telemetry(&self, telemetry: &EmbedderCacheTelemetry) -> EmbedResult<()> {
        telemetry.validate(EMBEDDER_CACHE_SCHEMA_VERSION, self.config)?;
        let bytes = serde_json::to_vec_pretty(telemetry).map_err(|err| {
            EmbedError::invalid(
                "EmbedderCacheTelemetry",
                format!("telemetry JSON serialization failed: {err}"),
                "ensure telemetry fields remain JSON serializable",
            )
        })?;
        write_bytes_atomic(&self.telemetry_path(), &bytes, EmbedderId::E1)
    }
    fn update_telemetry<F>(&self, update: F) -> EmbedResult<()>
    where
        F: FnOnce(&mut EmbedderCacheTelemetry) -> EmbedResult<()>,
    {
        let mut telemetry = self.load_telemetry()?;
        update(&mut telemetry)?;
        telemetry.last_event_unix_ms = unix_ms()?;
        self.write_telemetry(&telemetry)
    }
    fn update_telemetry_with_inventory<F>(&self, embedder: EmbedderId, update: F) -> EmbedResult<()>
    where
        F: FnOnce(&mut EmbedderCacheTelemetry) -> EmbedResult<()>,
    {
        let inventory = self.entry_records(embedder)?;
        let entry_count = inventory.len();
        let entry_bytes = inventory.iter().map(|entry| entry.byte_count).sum();
        self.update_telemetry(|telemetry| {
            update(telemetry)?;
            telemetry.entry_count = entry_count;
            telemetry.entry_bytes = entry_bytes;
            Ok(())
        })
    }
    fn record_hit(&self) -> EmbedResult<()> {
        self.update_telemetry_with_inventory(EmbedderId::E1, |t| {
            t.hits = checked_inc(t.hits, "EmbedderCacheTelemetry.hits")?;
            Ok(())
        })
    }
    fn record_miss(&self) -> EmbedResult<()> {
        self.update_telemetry_with_inventory(EmbedderId::E1, |t| {
            t.misses = checked_inc(t.misses, "EmbedderCacheTelemetry.misses")?;
            Ok(())
        })
    }
    fn record_write(&self) -> EmbedResult<()> {
        self.update_telemetry_with_inventory(EmbedderId::E1, |t| {
            t.writes = checked_inc(t.writes, "EmbedderCacheTelemetry.writes")?;
            Ok(())
        })
    }
    fn record_write_rejection(&self) -> EmbedResult<()> {
        self.update_telemetry_with_inventory(EmbedderId::E1, |t| {
            t.write_rejections = checked_inc(
                t.write_rejections,
                "EmbedderCacheTelemetry.write_rejections",
            )?;
            Ok(())
        })
    }
    fn record_eviction(&self, entries: u64, bytes: u64) -> EmbedResult<()> {
        self.update_telemetry_with_inventory(EmbedderId::E1, |t| {
            t.evictions = t.evictions.checked_add(entries).ok_or_else(|| {
                EmbedError::invalid(
                    "EmbedderCacheTelemetry.evictions",
                    "eviction counter overflowed u64",
                    "rotate telemetry after investigating pathological cache churn",
                )
            })?;
            t.evicted_bytes = t.evicted_bytes.checked_add(bytes).ok_or_else(|| {
                EmbedError::invalid(
                    "EmbedderCacheTelemetry.evicted_bytes",
                    "evicted_bytes counter overflowed u64",
                    "rotate telemetry after investigating pathological cache churn",
                )
            })?;
            Ok(())
        })
    }
    fn record_corrupt_entry_error(&self) -> EmbedResult<()> {
        self.update_telemetry(|t| {
            t.corrupt_entry_errors = checked_inc(
                t.corrupt_entry_errors,
                "EmbedderCacheTelemetry.corrupt_entry_errors",
            )?;
            Ok(())
        })
    }
    fn refresh_telemetry_inventory(&self) -> EmbedResult<()> {
        self.update_telemetry_with_inventory(EmbedderId::E1, |_| Ok(()))
    }
}
