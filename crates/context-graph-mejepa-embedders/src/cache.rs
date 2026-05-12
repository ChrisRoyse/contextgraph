use crate::embedder_id::EmbedderId;
use crate::error::{EmbedError, EmbedResult};
use crate::forward::EmbedderForward;
use crate::types::{EmbedderInput, EmbedderOutput};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CachedEmbedderVector {
    pub key: EmbedderCacheKey,
    pub vector: Vec<f32>,
    pub precision_class: String,
    pub written_at_unix_ms: u128,
}

impl CachedEmbedderVector {
    fn from_output(key: EmbedderCacheKey, output: &EmbedderOutput) -> EmbedResult<Self> {
        output.validate()?;
        validate_output_matches_key(&key, output)?;
        Ok(Self {
            key,
            vector: output.vector.clone(),
            precision_class: output.precision_class.clone(),
            written_at_unix_ms: unix_ms()?,
        })
    }

    fn validate(&self) -> EmbedResult<()> {
        self.key.validate()?;
        if self.vector.len() != self.key.embedder.dimension() {
            return Err(EmbedError::invalid(
                "CachedEmbedderVector.vector",
                format!(
                    "{} cached vector dimension {}, expected {}",
                    self.key.embedder,
                    self.vector.len(),
                    self.key.embedder.dimension()
                ),
                "delete the corrupt cache entry and recompute it from the pinned model",
            ));
        }
        if !self.vector.iter().all(|value| value.is_finite()) {
            return Err(EmbedError::invalid(
                "CachedEmbedderVector.vector",
                "cached vector contains NaN or Inf",
                "delete the corrupt cache entry and inspect the model forward logs",
            ));
        }
        if self.precision_class.trim().is_empty() {
            return Err(EmbedError::invalid(
                "CachedEmbedderVector.precision_class",
                "precision_class is empty",
                "persist the concrete forward precision class with each cache row",
            ));
        }
        Ok(())
    }

    fn into_output(self, source_id: String) -> EmbedResult<EmbedderOutput> {
        self.validate()?;
        let output = EmbedderOutput {
            embedder: self.key.embedder,
            source_id,
            vector: self.vector,
            model_version: self.key.model_version,
            precision_class: self.precision_class,
        };
        output.validate()?;
        Ok(output)
    }
}

#[derive(Debug, Clone)]
pub struct EmbedderCache {
    root: PathBuf,
}

impl EmbedderCache {
    pub fn new(root: impl Into<PathBuf>) -> EmbedResult<Self> {
        let root = root.into();
        if root.as_os_str().is_empty() {
            return Err(EmbedError::invalid(
                "EmbedderCache.root",
                "cache root path is empty",
                "configure a concrete local cache directory on durable storage",
            ));
        }
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn get(
        &self,
        key: &EmbedderCacheKey,
        source_id: String,
    ) -> EmbedResult<Option<EmbedderOutput>> {
        key.validate()?;
        let path = self.entry_path(key);
        if !path.exists() {
            return Ok(None);
        }
        let bytes = fs::read(&path).map_err(|err| {
            EmbedError::forward(
                key.embedder,
                format!("cache read failed at {}: {err}", path.display()),
                "inspect cache filesystem permissions and storage health; corrupt cache reads are not treated as misses",
            )
        })?;
        let cached: CachedEmbedderVector = serde_json::from_slice(&bytes).map_err(|err| {
            EmbedError::forward(
                key.embedder,
                format!("cache JSON parse failed at {}: {err}", path.display()),
                "delete the corrupt cache entry and recompute it from the pinned model",
            )
        })?;
        if cached.key != *key {
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
        let parent = path.parent().ok_or_else(|| {
            EmbedError::forward(
                key.embedder,
                format!("cache entry path has no parent: {}", path.display()),
                "configure a normal directory path for the embedder cache root",
            )
        })?;
        fs::create_dir_all(parent).map_err(|err| {
            EmbedError::forward(
                key.embedder,
                format!(
                    "cache directory create failed at {}: {err}",
                    parent.display()
                ),
                "inspect cache root permissions and available disk",
            )
        })?;
        let bytes = serde_json::to_vec(&cached).map_err(|err| {
            EmbedError::forward(
                key.embedder,
                format!("cache JSON serialization failed: {err}"),
                "ensure cached vector metadata remains JSON serializable",
            )
        })?;
        let tmp_path = path.with_extension("json.tmp");
        {
            let mut file = OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .open(&tmp_path)
                .map_err(|err| {
                    EmbedError::forward(
                        key.embedder,
                        format!("cache temp open failed at {}: {err}", tmp_path.display()),
                        "inspect cache root permissions and available disk",
                    )
                })?;
            file.write_all(&bytes).map_err(|err| {
                EmbedError::forward(
                    key.embedder,
                    format!("cache temp write failed at {}: {err}", tmp_path.display()),
                    "inspect cache root permissions and available disk",
                )
            })?;
            file.sync_all().map_err(|err| {
                EmbedError::forward(
                    key.embedder,
                    format!("cache temp fsync failed at {}: {err}", tmp_path.display()),
                    "inspect filesystem durability support and disk health",
                )
            })?;
        }
        fs::rename(&tmp_path, &path).map_err(|err| {
            EmbedError::forward(
                key.embedder,
                format!(
                    "cache atomic rename failed from {} to {}: {err}",
                    tmp_path.display(),
                    path.display()
                ),
                "ensure the cache temp file and final entry live on the same filesystem",
            )
        })?;
        fsync_dir(parent, key.embedder)?;
        let readback = self.get(key, output.source_id.clone())?.ok_or_else(|| {
            EmbedError::forward(
                key.embedder,
                format!(
                    "cache readback returned None after write at {}",
                    path.display()
                ),
                "inspect the cache key path derivation and filesystem visibility",
            )
        })?;
        if readback.vector != output.vector
            || readback.model_version != output.model_version
            || readback.precision_class != output.precision_class
        {
            return Err(EmbedError::forward(
                key.embedder,
                "cache readback did not match the written embedder output",
                "delete the corrupt cache entry and inspect disk write ordering",
            ));
        }
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
        let model_version_hash = chunk_sha256(&key.model_version);
        self.root
            .join(format!("{:?}", key.embedder))
            .join(model_version_hash)
            .join(format!("{}.json", key.chunk_sha256))
    }
}

pub fn chunk_sha256(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    hex_lower(hasher.finalize().as_slice())
}

fn validate_output_matches_key(key: &EmbedderCacheKey, output: &EmbedderOutput) -> EmbedResult<()> {
    if output.embedder != key.embedder {
        return Err(EmbedError::forward(
            key.embedder,
            format!(
                "cache output embedder mismatch: key={} output={}",
                key.embedder, output.embedder
            ),
            "write only outputs generated by the same embedder as the cache key",
        ));
    }
    if output.model_version != key.model_version {
        return Err(EmbedError::forward(
            key.embedder,
            format!(
                "cache output model_version mismatch: key={} output={}",
                key.model_version, output.model_version
            ),
            "write only outputs generated by the exact model version in the cache key",
        ));
    }
    Ok(())
}

fn fsync_dir(path: &Path, embedder: EmbedderId) -> EmbedResult<()> {
    let dir = OpenOptions::new().read(true).open(path).map_err(|err| {
        EmbedError::forward(
            embedder,
            format!(
                "cache directory open for fsync failed at {}: {err}",
                path.display()
            ),
            "inspect cache root permissions and filesystem support for directory fsync",
        )
    })?;
    dir.sync_all().map_err(|err| {
        EmbedError::forward(
            embedder,
            format!("cache directory fsync failed at {}: {err}", path.display()),
            "inspect filesystem durability support and disk health",
        )
    })
}

fn unix_ms() -> EmbedResult<u128> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| {
            EmbedError::invalid(
                "system_time",
                format!("system clock is before UNIX_EPOCH: {err}"),
                "fix host clock before writing cache evidence",
            )
        })?
        .as_millis())
}

fn is_lower_hex_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::forward::AlgorithmicEmbedderForward;

    #[tokio::test]
    async fn cache_writes_and_reads_algorithmic_output_from_disk() {
        let tmp = tempfile::tempdir().unwrap();
        let cache = EmbedderCache::new(tmp.path()).unwrap();
        let forward = AlgorithmicEmbedderForward::load(EmbedderId::E9).unwrap();
        let input = EmbedderInput {
            embedder: EmbedderId::E9,
            text: "fn cache_add(a: i32, b: i32) -> i32 { a + b }".to_string(),
            source_id: "src/cache.rs#0".to_string(),
        };

        let first = cache.forward_cached(&forward, &input).await.unwrap();
        let key = EmbedderCacheKey::for_input(&forward, &input).unwrap();
        assert!(cache.entry_path(&key).is_file());

        let second = cache.forward_cached(&forward, &input).await.unwrap();
        assert_eq!(first.vector, second.vector);
        assert_eq!(second.source_id, input.source_id);
    }

    #[test]
    fn cache_key_rejects_invalid_chunk_sha() {
        let key = EmbedderCacheKey::new(EmbedderId::E1, "v1", "not-a-sha".to_string());
        let err = key.validate().unwrap_err();
        assert_eq!(err.code(), "MEJEPA_EMBED_INVALID_INPUT");
    }
}
