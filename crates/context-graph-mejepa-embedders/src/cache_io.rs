use crate::embedder_id::EmbedderId;
use crate::error::{EmbedError, EmbedResult};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) fn write_bytes_atomic(
    path: &Path,
    bytes: &[u8],
    embedder: EmbedderId,
) -> EmbedResult<()> {
    let parent = path.parent().ok_or_else(|| {
        EmbedError::forward(
            embedder,
            format!("cache path has no parent: {}", path.display()),
            "configure a normal directory path for the embedder cache root",
        )
    })?;
    fs::create_dir_all(parent).map_err(|err| {
        EmbedError::forward(
            embedder,
            format!(
                "cache directory create failed at {}: {err}",
                parent.display()
            ),
            "inspect cache root permissions and available disk",
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
                    embedder,
                    format!("cache temp open failed at {}: {err}", tmp_path.display()),
                    "inspect cache root permissions and available disk",
                )
            })?;
        file.write_all(bytes).map_err(|err| {
            EmbedError::forward(
                embedder,
                format!("cache temp write failed at {}: {err}", tmp_path.display()),
                "inspect cache root permissions and available disk",
            )
        })?;
        file.sync_all().map_err(|err| {
            EmbedError::forward(
                embedder,
                format!("cache temp fsync failed at {}: {err}", tmp_path.display()),
                "inspect filesystem durability support and disk health",
            )
        })?;
    }
    fs::rename(&tmp_path, path).map_err(|err| {
        EmbedError::forward(
            embedder,
            format!(
                "cache atomic rename failed from {} to {}: {err}",
                tmp_path.display(),
                path.display()
            ),
            "ensure the cache temp file and final entry live on the same filesystem",
        )
    })?;
    fsync_dir(parent, embedder)
}

pub(crate) fn fsync_dir(path: &Path, embedder: EmbedderId) -> EmbedResult<()> {
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

pub(crate) fn unix_ms() -> EmbedResult<u128> {
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

pub(crate) fn is_lower_hex_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

pub(crate) fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

pub(crate) fn visit_cache_files<F>(path: &Path, f: &mut F) -> EmbedResult<()>
where
    F: FnMut(&Path) -> EmbedResult<()>,
{
    for entry in fs::read_dir(path).map_err(|err| {
        EmbedError::invalid(
            "EmbedderCache.root",
            format!("cache directory read failed at {}: {err}", path.display()),
            "inspect cache root permissions before scanning cache inventory",
        )
    })? {
        let path = entry
            .map_err(|err| {
                EmbedError::invalid(
                    "EmbedderCache.root",
                    format!(
                        "cache directory entry read failed at {}: {err}",
                        path.display()
                    ),
                    "inspect cache root permissions before scanning cache inventory",
                )
            })?
            .path();
        if path.is_dir() {
            visit_cache_files(&path, f)?;
        } else {
            f(&path)?;
        }
    }
    Ok(())
}

pub(crate) fn checked_inc(value: u64, field: &'static str) -> EmbedResult<u64> {
    value.checked_add(1).ok_or_else(|| {
        EmbedError::invalid(
            field,
            "telemetry counter overflowed u64",
            "rotate telemetry after investigating pathological cache activity",
        )
    })
}

pub(crate) fn cache_json_parse_error(
    embedder: EmbedderId,
    path: &Path,
    err: serde_json::Error,
) -> EmbedError {
    EmbedError::forward(
        embedder,
        format!("cache JSON parse failed at {}: {err}", path.display()),
        "delete the corrupt cache entry and recompute it from the pinned model",
    )
}
