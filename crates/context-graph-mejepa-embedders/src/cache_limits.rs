use crate::error::{EmbedError, EmbedResult};
use serde::{Deserialize, Serialize};
use std::env;

pub const EMBEDDER_CACHE_DEFAULT_MAX_ENTRIES: usize = 8192;
pub const EMBEDDER_CACHE_DEFAULT_MAX_BYTES: u64 = 4 * 1024 * 1024 * 1024;
pub const EMBEDDER_CACHE_TELEMETRY_FILE: &str = ".embedder-cache-telemetry.json";
pub const ENV_EMBEDDER_CACHE_MAX_ENTRIES: &str = "CG_MEJEPA_EMBED_CACHE_CAP";
pub const ENV_EMBEDDER_CACHE_MAX_BYTES: &str = "CG_MEJEPA_EMBED_CACHE_MAX_BYTES";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmbedderCacheConfig {
    pub max_entries: usize,
    pub max_bytes: u64,
}

impl Default for EmbedderCacheConfig {
    fn default() -> Self {
        Self {
            max_entries: EMBEDDER_CACHE_DEFAULT_MAX_ENTRIES,
            max_bytes: EMBEDDER_CACHE_DEFAULT_MAX_BYTES,
        }
    }
}

impl EmbedderCacheConfig {
    pub fn from_env() -> EmbedResult<Self> {
        let mut config = Self::default();
        if let Ok(value) = env::var(ENV_EMBEDDER_CACHE_MAX_ENTRIES) {
            config.max_entries = parse_positive_usize(ENV_EMBEDDER_CACHE_MAX_ENTRIES, &value)?;
        }
        if let Ok(value) = env::var(ENV_EMBEDDER_CACHE_MAX_BYTES) {
            config.max_bytes = parse_positive_u64(ENV_EMBEDDER_CACHE_MAX_BYTES, &value)?;
        }
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> EmbedResult<()> {
        if self.max_entries == 0 {
            return Err(EmbedError::invalid(
                "EmbedderCacheConfig.max_entries",
                "max_entries must be >= 1",
                "set CG_MEJEPA_EMBED_CACHE_CAP to a positive integer",
            ));
        }
        if self.max_bytes == 0 {
            return Err(EmbedError::invalid(
                "EmbedderCacheConfig.max_bytes",
                "max_bytes must be >= 1",
                "set CG_MEJEPA_EMBED_CACHE_MAX_BYTES to a positive integer",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmbedderCacheTelemetry {
    pub schema_version: u16,
    pub max_entries: usize,
    pub max_bytes: u64,
    pub hits: u64,
    pub misses: u64,
    pub writes: u64,
    pub write_rejections: u64,
    pub evictions: u64,
    pub evicted_bytes: u64,
    pub corrupt_entry_errors: u64,
    pub entry_count: usize,
    pub entry_bytes: u64,
    pub last_event_unix_ms: u128,
}

impl EmbedderCacheTelemetry {
    pub(crate) fn new(schema_version: u16, config: EmbedderCacheConfig, now: u128) -> Self {
        Self {
            schema_version,
            max_entries: config.max_entries,
            max_bytes: config.max_bytes,
            hits: 0,
            misses: 0,
            writes: 0,
            write_rejections: 0,
            evictions: 0,
            evicted_bytes: 0,
            corrupt_entry_errors: 0,
            entry_count: 0,
            entry_bytes: 0,
            last_event_unix_ms: now,
        }
    }

    pub(crate) fn validate(
        &self,
        schema_version: u16,
        config: EmbedderCacheConfig,
    ) -> EmbedResult<()> {
        if self.schema_version != schema_version {
            return Err(EmbedError::invalid(
                "EmbedderCacheTelemetry.schema_version",
                format!(
                    "telemetry schema_version {} does not match runtime schema {}",
                    self.schema_version, schema_version
                ),
                "delete or migrate the telemetry file before using this cache root",
            ));
        }
        if self.max_entries != config.max_entries || self.max_bytes != config.max_bytes {
            return Err(EmbedError::invalid(
                "EmbedderCacheTelemetry.config",
                format!(
                    "telemetry config max_entries={} max_bytes={} does not match runtime max_entries={} max_bytes={}",
                    self.max_entries, self.max_bytes, config.max_entries, config.max_bytes
                ),
                "start a new cache root or explicitly reconcile the telemetry/config mismatch",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct PruneReport {
    pub before_entries: usize,
    pub before_bytes: u64,
    pub after_entries: usize,
    pub after_bytes: u64,
    pub evicted_entries: usize,
    pub evicted_bytes: u64,
    pub evicted_paths: Vec<String>,
}

fn parse_positive_usize(name: &'static str, value: &str) -> EmbedResult<usize> {
    let parsed = value.parse::<usize>().map_err(|err| {
        EmbedError::invalid(
            name,
            format!("{name} must parse as a positive integer; got {value:?}: {err}"),
            "fix the cache capacity environment variable before starting ME-JEPA",
        )
    })?;
    if parsed == 0 {
        return Err(EmbedError::invalid(
            name,
            format!("{name} must be >= 1; got 0"),
            "fix the cache capacity environment variable before starting ME-JEPA",
        ));
    }
    Ok(parsed)
}

fn parse_positive_u64(name: &'static str, value: &str) -> EmbedResult<u64> {
    let parsed = value.parse::<u64>().map_err(|err| {
        EmbedError::invalid(
            name,
            format!("{name} must parse as a positive integer; got {value:?}: {err}"),
            "fix the cache byte-limit environment variable before starting ME-JEPA",
        )
    })?;
    if parsed == 0 {
        return Err(EmbedError::invalid(
            name,
            format!("{name} must be >= 1; got 0"),
            "fix the cache byte-limit environment variable before starting ME-JEPA",
        ));
    }
    Ok(parsed)
}
