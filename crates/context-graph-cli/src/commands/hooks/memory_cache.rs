//! Filesystem-based memory cache for cross-process context injection.
//!
//! # Purpose
//! pre_tool_use has a 500ms total timeout (per constitution.yaml) with ~100ms CLI logic target.
//! This budget is too tight for MCP network calls. This cache stores memories retrieved during
//! user_prompt_submit (2s budget) so they can be accessed instantly by pre_tool_use.
//!
//! # Architecture
//! - Each hook invocation is a separate CLI process (AP-50: shell scripts call CLI)
//! - In-memory OnceLock singletons DON'T work across process boundaries
//! - This module uses filesystem-based caching at /tmp/cg-memory-cache-{session_id}.json
//! - user_prompt_submit writes the cache file
//! - pre_tool_use and inject-brief read the cache file
//! - session_end can clean up cache files
//!
//! # Constitution References
//! - AP-50: NO internal hooks - shell scripts call CLI
//! - hooks.timeout_ms.pre_tool_use: 500ms total (FAST PATH, CLI logic ~100ms)

use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

/// Cache TTL - memories expire after 5 minutes.
const CACHE_TTL: Duration = Duration::from_secs(300);

/// Maximum memories to cache per session.
const MAX_MEMORIES_PER_SESSION: usize = 10;

/// A cached memory with content and similarity score.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedMemory {
    /// Memory content text.
    pub content: String,
    /// Similarity score to the query.
    pub similarity: f32,
}

/// On-disk cache entry with expiration timestamp.
#[derive(Debug, Serialize, Deserialize)]
struct CacheFile {
    /// Cached memories.
    memories: Vec<CachedMemory>,
    /// Unix timestamp (seconds) when the entry was created.
    created_at_secs: u64,
}

impl CacheFile {
    fn new(memories: Vec<CachedMemory>) -> Self {
        let created_at_secs = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        Self {
            memories,
            created_at_secs,
        }
    }

    fn is_expired(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        now.saturating_sub(self.created_at_secs) > CACHE_TTL.as_secs()
    }
}

/// Get the cache file path for a session.
fn cache_path(session_id: &str) -> PathBuf {
    // Sanitize session_id to prevent path traversal
    let safe_id: String = session_id
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
        .take(128)
        .collect();

    let dir = std::env::temp_dir().join("cg-memory-cache");
    dir.join(format!("{safe_id}.json"))
}

/// Ensure the cache directory exists.
fn ensure_cache_dir() -> std::io::Result<()> {
    let dir = std::env::temp_dir().join("cg-memory-cache");
    std::fs::create_dir_all(dir)
}

// =============================================================================
// Public API
// =============================================================================

/// Store memories in the filesystem cache for a session.
///
/// Called by user_prompt_submit after retrieving memories from MCP.
///
/// # Arguments
/// * `session_id` - Session identifier
/// * `memories` - Retrieved memories to cache
pub fn cache_memories(session_id: &str, memories: Vec<CachedMemory>) {
    let memory_count = memories.len();

    // Limit memories per session
    let memories = if memories.len() > MAX_MEMORIES_PER_SESSION {
        memories.into_iter().take(MAX_MEMORIES_PER_SESSION).collect()
    } else {
        memories
    };

    let cache_file = CacheFile::new(memories);

    if let Err(e) = ensure_cache_dir() {
        tracing::error!(error = %e, "MEMORY_CACHE: Failed to create cache directory");
        return;
    }

    let path = cache_path(session_id);

    // Write atomically: write to temp file then rename
    let tmp_path = path.with_extension("tmp");
    match serde_json::to_vec(&cache_file) {
        Ok(data) => {
            match std::fs::File::create(&tmp_path) {
                Ok(mut f) => {
                    if let Err(e) = f.write_all(&data) {
                        tracing::error!(error = %e, "MEMORY_CACHE: Failed to write cache file");
                        let _ = std::fs::remove_file(&tmp_path);
                        return;
                    }
                }
                Err(e) => {
                    tracing::error!(error = %e, "MEMORY_CACHE: Failed to create cache file");
                    return;
                }
            }
            if let Err(e) = std::fs::rename(&tmp_path, &path) {
                tracing::error!(error = %e, "MEMORY_CACHE: Failed to rename cache file");
                let _ = std::fs::remove_file(&tmp_path);
                return;
            }
        }
        Err(e) => {
            tracing::error!(error = %e, "MEMORY_CACHE: Failed to serialize cache");
            return;
        }
    }

    tracing::debug!(
        session_id,
        memory_count,
        path = %path.display(),
        "MEMORY_CACHE: Stored memories to filesystem"
    );
}

/// Get cached memories for a session from the filesystem.
///
/// Called by pre_tool_use and inject-brief to get memories without MCP calls.
///
/// # Arguments
/// * `session_id` - Session identifier
///
/// # Returns
/// * Cached memories if available and not expired, empty vec otherwise
pub fn get_cached_memories(session_id: &str) -> Vec<CachedMemory> {
    let path = cache_path(session_id);

    let data = match std::fs::read(&path) {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };

    let cache_file: CacheFile = match serde_json::from_slice(&data) {
        Ok(cf) => cf,
        Err(e) => {
            tracing::warn!(error = %e, "MEMORY_CACHE: Corrupted cache file, ignoring");
            let _ = std::fs::remove_file(&path);
            return Vec::new();
        }
    };

    if cache_file.is_expired() {
        tracing::debug!(session_id, "MEMORY_CACHE: Cache expired, removing");
        let _ = std::fs::remove_file(&path);
        return Vec::new();
    }

    tracing::debug!(
        session_id,
        memory_count = cache_file.memories.len(),
        "MEMORY_CACHE: Retrieved cached memories from filesystem"
    );

    cache_file.memories
}

/// Clean up cache file for a session.
///
/// Called by session_end hook to remove stale cache files.
pub fn clear_session_cache(session_id: &str) {
    let path = cache_path(session_id);
    if path.exists() {
        if let Err(e) = std::fs::remove_file(&path) {
            tracing::warn!(error = %e, session_id, "MEMORY_CACHE: Failed to clean up cache file");
        } else {
            tracing::debug!(session_id, "MEMORY_CACHE: Cleaned up cache file");
        }
    }
}

/// Clean up all expired cache files.
///
/// Can be called periodically or at startup.
#[allow(dead_code)]
pub fn clear_expired_caches() {
    let dir = std::env::temp_dir().join("cg-memory-cache");
    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    let mut removed = 0usize;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        if let Ok(data) = std::fs::read(&path) {
            if let Ok(cache_file) = serde_json::from_slice::<CacheFile>(&data) {
                if cache_file.is_expired() {
                    let _ = std::fs::remove_file(&path);
                    removed += 1;
                }
            }
        }
    }
    if removed > 0 {
        tracing::debug!(removed, "MEMORY_CACHE: Cleaned up expired cache files");
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn test_session_id() -> String {
        format!("test-session-{}", std::process::id())
    }

    #[test]
    fn test_cache_and_retrieve_memories() {
        let session_id = test_session_id();
        let memories = vec![
            CachedMemory {
                content: "Test memory content".to_string(),
                similarity: 0.85,
            },
            CachedMemory {
                content: "Another memory".to_string(),
                similarity: 0.72,
            },
        ];

        cache_memories(&session_id, memories);

        let retrieved = get_cached_memories(&session_id);
        assert_eq!(retrieved.len(), 2);
        assert_eq!(retrieved[0].content, "Test memory content");
        assert_eq!(retrieved[0].similarity, 0.85);

        // Clean up
        clear_session_cache(&session_id);
    }

    #[test]
    fn test_empty_cache_returns_empty() {
        let retrieved = get_cached_memories("nonexistent-session-xyz");
        assert!(retrieved.is_empty());
    }

    #[test]
    fn test_clear_session_cache() {
        let session_id = format!("test-clear-{}", std::process::id());
        let memories = vec![CachedMemory {
            content: "to be cleared".to_string(),
            similarity: 0.5,
        }];

        cache_memories(&session_id, memories);
        assert!(!get_cached_memories(&session_id).is_empty());

        clear_session_cache(&session_id);
        assert!(get_cached_memories(&session_id).is_empty());
    }

    #[test]
    fn test_max_memories_per_session() {
        let session_id = format!("test-max-{}", std::process::id());
        let memories: Vec<CachedMemory> = (0..20)
            .map(|i| CachedMemory {
                content: format!("memory {i}"),
                similarity: 0.5,
            })
            .collect();

        cache_memories(&session_id, memories);

        let retrieved = get_cached_memories(&session_id);
        assert_eq!(retrieved.len(), MAX_MEMORIES_PER_SESSION);

        // Clean up
        clear_session_cache(&session_id);
    }

    #[test]
    fn test_cache_path_sanitization() {
        // Path traversal attempt should be sanitized
        let path = cache_path("../../../etc/passwd");
        assert!(!path.to_str().unwrap().contains(".."));
        assert!(!path.to_str().unwrap().contains("etc/passwd"));
    }

    #[test]
    fn test_overwrite_existing_cache() {
        let session_id = format!("test-overwrite-{}", std::process::id());

        cache_memories(
            &session_id,
            vec![CachedMemory {
                content: "first".to_string(),
                similarity: 0.5,
            }],
        );

        cache_memories(
            &session_id,
            vec![CachedMemory {
                content: "second".to_string(),
                similarity: 0.9,
            }],
        );

        let retrieved = get_cached_memories(&session_id);
        assert_eq!(retrieved.len(), 1);
        assert_eq!(retrieved[0].content, "second");

        // Clean up
        clear_session_cache(&session_id);
    }
}
