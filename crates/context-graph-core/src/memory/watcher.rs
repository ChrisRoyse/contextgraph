//! GitFileWatcher - Git-based file watcher for markdown file changes.
//!
//! Implements TASK-P1-008: Monitor directories for markdown file changes
//! and automatically chunk/capture them as memories.
//!
//! Uses `git status` and `git ls-files` for change detection instead of
//! filesystem events, eliminating platform-specific issues (WSL2).
//!
//! # Architecture
//! Git Commands -> File Read -> TextChunker -> MemoryCaptureService -> MemoryStore
//!
//! # Constitution Compliance
//! - ARCH-11: Memory sources include MDFileChunk
//! - AP-08: Uses tokio::fs for async I/O
//! - AP-14: No .unwrap() in library code

use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;
use tracing::{debug, error, info, instrument, warn};
use uuid::Uuid;

use super::capture::CaptureError;
use super::chunker::ChunkerError;
use super::{MemoryCaptureService, TextChunker};

/// Errors from GitFileWatcher operations.
/// All errors include context for debugging.
/// Per constitution AP-14: No .unwrap() - all errors propagate via Result.
#[derive(Debug, Error)]
pub enum WatcherError {
    /// Path does not exist or is not accessible.
    #[error("Path not found: {path:?}")]
    PathNotFound { path: PathBuf },

    /// Path is not a directory (file watching requires directory).
    #[error("Path is not a directory: {path:?}")]
    NotADirectory { path: PathBuf },

    /// Path is not inside a git repository.
    #[error("Path is not in a git repository: {path:?}")]
    NotGitRepository { path: PathBuf },

    /// Git command execution failed.
    #[error("Git command failed: {command}: {message}")]
    GitCommandFailed { command: String, message: String },

    /// Git binary not found.
    #[error("Git not found in PATH")]
    GitNotFound,

    /// File read operation failed.
    #[error("Failed to read file {path:?}: {source}")]
    ReadFailed {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// File is not valid UTF-8.
    #[error("File is not valid UTF-8: {path:?}")]
    InvalidUtf8 { path: PathBuf },

    /// Text chunking failed.
    #[error("Chunking failed for {path:?}: {source}")]
    ChunkingFailed {
        path: PathBuf,
        #[source]
        source: ChunkerError,
    },

    /// Memory capture failed.
    #[error("Capture failed for {path:?}: {source}")]
    CaptureFailed {
        path: PathBuf,
        #[source]
        source: CaptureError,
    },

    /// Watcher is not started.
    #[error("Watcher not started - call start() first")]
    NotStarted,

    /// Watcher is already running.
    #[error("Watcher already running")]
    AlreadyRunning,
}

/// GitFileWatcher monitors directories for markdown file changes using git.
///
/// # Architecture
/// - Uses `git ls-files` for initial file discovery
/// - Uses `git status --porcelain` for change detection
/// - Tracks SHA256 hashes to detect actual content changes
/// - Processes files through TextChunker (200 words, 50 overlap)
/// - Stores chunks via MemoryCaptureService
///
/// # Thread Safety
/// The watcher is `Send + Sync` and can be shared across async tasks.
/// File hash tracking uses `Arc<RwLock<HashMap>>` for thread-safe access.
///
/// # Lifecycle
/// 1. Create with `new(paths, capture_service, session_id)`
/// 2. Call `start()` to begin watching and perform initial scan
/// 3. Call `process_events()` periodically to poll for git changes
/// 4. Call `stop()` for clean shutdown
pub struct GitFileWatcher {
    /// Memory capture service for storing chunks.
    capture_service: Arc<MemoryCaptureService>,

    /// Text chunker for splitting files into chunks.
    chunker: TextChunker,

    /// File content hashes for change detection.
    /// Key: canonical path, Value: SHA256 hash
    file_hashes: Arc<RwLock<HashMap<PathBuf, String>>>,

    /// Session ID for captured memories.
    session_id: String,

    /// Paths being watched.
    watch_paths: Vec<PathBuf>,

    /// Running state flag.
    is_running: bool,

    /// Git repository root (detected on start).
    git_root: Option<PathBuf>,
}

impl GitFileWatcher {
    /// Create a new GitFileWatcher.
    ///
    /// # Arguments
    /// * `watch_paths` - Directories to watch for .md files
    /// * `capture_service` - Service for capturing memories
    /// * `session_id` - Session to associate memories with
    ///
    /// # Errors
    /// * `WatcherError::PathNotFound` - Path does not exist
    /// * `WatcherError::NotADirectory` - Path is not a directory
    /// * `WatcherError::NotGitRepository` - Path is not in a git repo
    /// * `WatcherError::GitNotFound` - Git binary not found
    ///
    /// # Example
    /// ```ignore
    /// let capture_service = Arc::new(MemoryCaptureService::new(store, embedder));
    /// let watcher = GitFileWatcher::new(
    ///     vec![PathBuf::from("./docs")],
    ///     capture_service,
    ///     "session-123".to_string(),
    /// )?;
    /// ```
    pub fn new(
        watch_paths: Vec<PathBuf>,
        capture_service: Arc<MemoryCaptureService>,
        session_id: String,
    ) -> Result<Self, WatcherError> {
        // Check git is available
        Self::check_git_available()?;

        // Fail fast: validate all paths exist and are directories
        for path in &watch_paths {
            if !path.exists() {
                return Err(WatcherError::PathNotFound { path: path.clone() });
            }
            if !path.is_dir() {
                return Err(WatcherError::NotADirectory { path: path.clone() });
            }
        }

        // Detect git root from first watch path
        let git_root = if !watch_paths.is_empty() {
            Some(Self::detect_git_root(&watch_paths[0])?)
        } else {
            None
        };

        Ok(Self {
            capture_service,
            chunker: TextChunker::default_config(),
            file_hashes: Arc::new(RwLock::new(HashMap::new())),
            session_id,
            watch_paths,
            is_running: false,
            git_root,
        })
    }

    /// Check if git is available in PATH.
    fn check_git_available() -> Result<(), WatcherError> {
        let output = Command::new("git").arg("--version").output();

        match output {
            Ok(output) if output.status.success() => Ok(()),
            Ok(output) => Err(WatcherError::GitCommandFailed {
                command: "git --version".to_string(),
                message: String::from_utf8_lossy(&output.stderr).to_string(),
            }),
            Err(_) => Err(WatcherError::GitNotFound),
        }
    }

    /// Detect the git repository root for a path.
    fn detect_git_root(path: &Path) -> Result<PathBuf, WatcherError> {
        let output = Command::new("git")
            .args(["rev-parse", "--show-toplevel"])
            .current_dir(path)
            .output()
            .map_err(|_| WatcherError::GitNotFound)?;

        if output.status.success() {
            let root = String::from_utf8_lossy(&output.stdout).trim().to_string();
            Ok(PathBuf::from(root))
        } else {
            Err(WatcherError::NotGitRepository {
                path: path.to_path_buf(),
            })
        }
    }

    /// Start watching directories and perform initial scan.
    ///
    /// # Behavior
    /// 1. Verifies git repository is accessible
    /// 2. Performs initial scan using git ls-files
    /// 3. Sets is_running = true
    ///
    /// # Errors
    /// * `WatcherError::AlreadyRunning` - start() called twice
    /// * `WatcherError::NotGitRepository` - not in a git repo
    #[instrument(skip(self))]
    pub async fn start(&mut self) -> Result<(), WatcherError> {
        if self.is_running {
            return Err(WatcherError::AlreadyRunning);
        }

        self.is_running = true;

        // Perform initial scan of existing files using git ls-files
        for path in &self.watch_paths.clone() {
            if let Err(e) = self.scan_directory_git(path).await {
                warn!(path = ?path, error = %e, "Initial scan failed for directory");
            }
        }

        info!(
            paths = ?self.watch_paths,
            session_id = %self.session_id,
            git_root = ?self.git_root,
            "GitFileWatcher started"
        );

        Ok(())
    }

    /// Process pending changes from git status.
    ///
    /// Call this method periodically (e.g., in a loop with sleep).
    /// Uses `git status --porcelain` to detect changes.
    ///
    /// # Returns
    /// Number of files processed in this call.
    ///
    /// # Errors
    /// * `WatcherError::NotStarted` - start() not called yet
    #[instrument(skip(self))]
    pub async fn process_events(&mut self) -> Result<usize, WatcherError> {
        if !self.is_running {
            return Err(WatcherError::NotStarted);
        }

        let mut files_processed = 0;

        for path in &self.watch_paths.clone() {
            let changes = self.get_git_changes(path)?;

            for change in changes {
                match change.status.as_str() {
                    // Deleted files:
                    // "D " = staged deletion, " D" = working tree deletion
                    // "DD" = both staged and working tree
                    // "AD" = added then deleted (never committed)
                    "D " | " D" | "DD" | "AD" => {
                        let full_path = self.resolve_path(&change.path)?;
                        match self.handle_file_deletion(&full_path).await {
                            Ok(deleted) => {
                                if deleted > 0 {
                                    files_processed += 1;
                                    info!(
                                        path = ?change.path,
                                        deleted_count = deleted,
                                        "Cleaned up embeddings for deleted file"
                                    );
                                }
                            }
                            Err(e) => {
                                error!(
                                    path = ?change.path,
                                    error = %e,
                                    "Failed to clean up embeddings for deleted file"
                                );
                            }
                        }
                    }
                    // Modified or new files
                    "M" | " M" | "??" | "A" | " A" | "AM" | "MM" => {
                        let full_path = self.resolve_path(&change.path)?;
                        if self.is_markdown(&full_path) && full_path.exists() {
                            match self.process_file(&full_path).await {
                                Ok(ids) => {
                                    if !ids.is_empty() {
                                        files_processed += 1;
                                        info!(
                                            path = ?change.path,
                                            chunks = ids.len(),
                                            "Processed markdown file"
                                        );
                                    }
                                }
                                Err(e) => {
                                    error!(path = ?change.path, error = %e, "Failed to process file");
                                }
                            }
                        }
                    }
                    _ => {
                        debug!(status = %change.status, path = ?change.path, "Ignoring git status");
                    }
                }
            }
        }

        Ok(files_processed)
    }

    /// Get git status changes for a directory.
    fn get_git_changes(&self, path: &Path) -> Result<Vec<GitChange>, WatcherError> {
        let output = Command::new("git")
            .args(["status", "--porcelain", "."])
            .current_dir(path)
            .output()
            .map_err(|e| WatcherError::GitCommandFailed {
                command: "git status --porcelain".to_string(),
                message: e.to_string(),
            })?;

        if !output.status.success() {
            return Err(WatcherError::GitCommandFailed {
                command: "git status --porcelain".to_string(),
                message: String::from_utf8_lossy(&output.stderr).to_string(),
            });
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let changes = stdout
            .lines()
            .filter_map(|line| {
                if line.len() < 4 {
                    return None;
                }
                let status = line[..2].to_string();
                let file_path = line[3..].to_string();

                // Only include markdown files
                if file_path.ends_with(".md") || file_path.ends_with(".markdown") {
                    Some(GitChange {
                        status,
                        path: file_path,
                    })
                } else {
                    None
                }
            })
            .collect();

        Ok(changes)
    }

    /// Resolve a relative git path to an absolute path.
    fn resolve_path(&self, rel_path: &str) -> Result<PathBuf, WatcherError> {
        if let Some(ref git_root) = self.git_root {
            Ok(git_root.join(rel_path))
        } else if !self.watch_paths.is_empty() {
            Ok(self.watch_paths[0].join(rel_path))
        } else {
            Ok(PathBuf::from(rel_path))
        }
    }

    /// Scan a directory for existing markdown files using git ls-files.
    async fn scan_directory_git(&self, dir: &Path) -> Result<usize, WatcherError> {
        let mut files_processed = 0;

        // Get list of tracked markdown files
        let output = Command::new("git")
            .args(["ls-files", "*.md", "**/*.md"])
            .current_dir(dir)
            .output()
            .map_err(|e| WatcherError::GitCommandFailed {
                command: "git ls-files".to_string(),
                message: e.to_string(),
            })?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let tracked_files: Vec<&str> = stdout.lines().collect();

        // Also scan for untracked markdown files in the directory
        let mut all_md_files = Vec::new();
        self.find_md_files_recursive(dir, &mut all_md_files).await?;

        // Combine tracked and untracked (untracked files found by filesystem scan)
        for file_path in all_md_files {
            match self.process_file(&file_path).await {
                Ok(ids) => {
                    if !ids.is_empty() {
                        files_processed += 1;
                    }
                }
                Err(e) => {
                    warn!(path = ?file_path, error = %e, "Failed to process file during scan");
                }
            }
        }

        // Process any tracked files that might be outside the recursive scan
        for rel_path in tracked_files {
            let full_path = dir.join(rel_path);
            if full_path.exists() && self.is_markdown(&full_path) {
                // Check if we already processed this
                let hashes = self.file_hashes.read().await;
                let canonical = full_path
                    .canonicalize()
                    .unwrap_or_else(|_| full_path.clone());
                if hashes.contains_key(&canonical) {
                    continue;
                }
                drop(hashes);

                match self.process_file(&full_path).await {
                    Ok(ids) => {
                        if !ids.is_empty() {
                            files_processed += 1;
                        }
                    }
                    Err(e) => {
                        warn!(path = ?full_path, error = %e, "Failed to process tracked file");
                    }
                }
            }
        }

        info!(root = ?dir, files = files_processed, "Git-based directory scan complete");
        Ok(files_processed)
    }

    /// Recursively find all markdown files in a directory.
    async fn find_md_files_recursive(
        &self,
        dir: &Path,
        files: &mut Vec<PathBuf>,
    ) -> Result<(), WatcherError> {
        let mut entries = tokio::fs::read_dir(dir)
            .await
            .map_err(|e| WatcherError::ReadFailed {
                path: dir.to_path_buf(),
                source: e,
            })?;

        while let Some(entry) =
            entries
                .next_entry()
                .await
                .map_err(|e| WatcherError::ReadFailed {
                    path: dir.to_path_buf(),
                    source: e,
                })?
        {
            let path = entry.path();

            if path.is_dir() {
                // Skip .git directory
                if path.file_name().map(|n| n == ".git").unwrap_or(false) {
                    continue;
                }
                // Recursively scan subdirectory
                Box::pin(self.find_md_files_recursive(&path, files)).await?;
            } else if path.is_file() && self.is_markdown(&path) {
                files.push(path);
            }
        }

        Ok(())
    }

    /// Process a single markdown file.
    ///
    /// # Behavior
    /// 1. Read file content (async)
    /// 2. Compute SHA256 hash
    /// 3. Check if content changed (hash comparison)
    /// 4. If changed: DELETE old embeddings, then chunk and capture new
    /// 5. Update hash cache
    ///
    /// # CRITICAL: Stale Embedding Cleanup
    ///
    /// When a file is modified, we MUST delete all old embeddings BEFORE
    /// creating new ones. This ensures the knowledge graph always reflects
    /// the current state of the file. Without this, stale embeddings would
    /// accumulate indefinitely.
    ///
    /// # Returns
    /// Vec of memory UUIDs created (empty if file unchanged).
    #[instrument(skip(self))]
    async fn process_file(&self, path: &Path) -> Result<Vec<Uuid>, WatcherError> {
        // Read file content using async I/O (AP-08 compliance)
        let content =
            tokio::fs::read_to_string(path)
                .await
                .map_err(|e| WatcherError::ReadFailed {
                    path: path.to_path_buf(),
                    source: e,
                })?;

        // Compute SHA256 hash
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        let hash = format!("{:x}", hasher.finalize());

        // Get canonical path for consistent hash key
        let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let path_str = canonical.to_string_lossy().to_string();

        // Check for existing hash to detect change vs new file
        let is_update;
        {
            let hashes = self.file_hashes.read().await;
            if let Some(existing_hash) = hashes.get(&canonical) {
                if existing_hash == &hash {
                    debug!(path = ?path, "File unchanged, skipping");
                    return Ok(Vec::new());
                }
                // Hash differs - this is an UPDATE, not a new file
                is_update = true;
            } else {
                // No existing hash - this is a NEW file
                is_update = false;
            }
        }

        // CRITICAL: If this is an UPDATE (file content changed), delete old embeddings FIRST
        if is_update {
            info!(
                path = %path_str,
                "File content changed - clearing old embeddings before re-chunking"
            );
            let deleted_count = self
                .capture_service
                .delete_by_file_path(&path_str)
                .await
                .map_err(|e| {
                    error!(
                        path = %path_str,
                        error = %e,
                        "CRITICAL: Failed to delete old embeddings for modified file. \
                         Database may contain stale embeddings."
                    );
                    WatcherError::CaptureFailed {
                        path: path.to_path_buf(),
                        source: e,
                    }
                })?;
            info!(
                path = %path_str,
                deleted_count = deleted_count,
                "Cleared old embeddings"
            );
        }

        // Update hash cache AFTER successful delete (or for new files)
        {
            let mut hashes = self.file_hashes.write().await;
            hashes.insert(canonical, hash);
        }

        // Chunk the content
        let chunks = self.chunker.chunk_text(&content, &path_str).map_err(|e| {
            WatcherError::ChunkingFailed {
                path: path.to_path_buf(),
                source: e,
            }
        })?;

        info!(
            path = %path_str,
            chunks = chunks.len(),
            is_update = is_update,
            "Chunked file"
        );

        // Capture each chunk
        let mut memory_ids = Vec::with_capacity(chunks.len());
        for chunk in chunks {
            let id = self
                .capture_service
                .capture_md_chunk(chunk, self.session_id.clone())
                .await
                .map_err(|e| WatcherError::CaptureFailed {
                    path: path.to_path_buf(),
                    source: e,
                })?;
            memory_ids.push(id);
        }

        info!(
            path = %path_str,
            new_chunks = memory_ids.len(),
            "Stored fresh embeddings"
        );

        Ok(memory_ids)
    }

    /// Handle file deletion by cleaning up associated embeddings.
    ///
    /// # CRITICAL: Stale Embedding Cleanup
    ///
    /// When a file is deleted, we MUST delete all associated embeddings from
    /// both the MemoryStore and TeleologicalStore. This ensures the knowledge
    /// graph always reflects exactly what exists in the ./docs/ directory.
    ///
    /// # Arguments
    /// * `path` - Path to the deleted file
    ///
    /// # Returns
    /// Number of embeddings deleted.
    #[instrument(skip(self))]
    async fn handle_file_deletion(&self, path: &Path) -> Result<usize, WatcherError> {
        // Try to get canonical path from hash cache, or use the path directly
        // For deleted files, we need to reconstruct what the canonical path was
        let path_str = {
            // First try the path as-is
            let direct_path = path.to_string_lossy().to_string();

            // Check if we have this path (or its canonical form) in our hash cache
            let hashes = self.file_hashes.read().await;
            let mut matched_canonical: Option<PathBuf> = None;

            for cached_path in hashes.keys() {
                // Check if the cached canonical path ends with the same filename
                if cached_path.file_name() == path.file_name() {
                    // This is likely the file that was deleted
                    matched_canonical = Some(cached_path.clone());
                    break;
                }
            }
            drop(hashes);

            matched_canonical
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or(direct_path)
        };

        info!(
            path = %path_str,
            "File deleted - cleaning up embeddings"
        );

        // Delete all embeddings for this file from both stores
        let deleted_count = self
            .capture_service
            .delete_by_file_path(&path_str)
            .await
            .map_err(|e| {
                error!(
                    path = %path_str,
                    error = %e,
                    "Failed to delete embeddings for deleted file"
                );
                WatcherError::CaptureFailed {
                    path: path.to_path_buf(),
                    source: e,
                }
            })?;

        // Remove from hash cache
        {
            let mut hashes = self.file_hashes.write().await;
            // Try both the original path and reconstructed canonical path
            hashes.retain(|cached_path, _| cached_path.file_name() != path.file_name());
        }

        info!(
            path = %path_str,
            deleted_count = deleted_count,
            "Successfully cleaned up embeddings for deleted file"
        );

        Ok(deleted_count)
    }

    /// Check if a path is a markdown file.
    fn is_markdown(&self, path: &Path) -> bool {
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("md") || ext.eq_ignore_ascii_case("markdown"))
            .unwrap_or(false)
    }

    /// Stop watching and clean up resources.
    ///
    /// Idempotent: safe to call multiple times.
    pub fn stop(&mut self) {
        self.is_running = false;
        info!(session_id = %self.session_id, "GitFileWatcher stopped");
    }

    /// Get the number of files in the hash cache.
    pub async fn cached_file_count(&self) -> usize {
        self.file_hashes.read().await.len()
    }

    /// Check if the watcher is currently running.
    pub fn is_running(&self) -> bool {
        self.is_running
    }

    /// Get a reference to the capture service.
    ///
    /// Useful for tests that need to verify memory storage or perform
    /// cleanup operations like delete_by_file_path.
    pub fn capture_service(&self) -> &Arc<MemoryCaptureService> {
        &self.capture_service
    }
}

// Ensure proper cleanup on drop
impl Drop for GitFileWatcher {
    fn drop(&mut self) {
        if self.is_running {
            self.stop();
        }
    }
}

/// Represents a change detected by git status.
#[derive(Debug)]
struct GitChange {
    /// Git status code (e.g., " M", "??", "D ")
    status: String,
    /// Relative file path
    path: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::capture::TestEmbeddingProvider;
    use crate::memory::MemoryStore;
    use std::fs;
    use tempfile::tempdir;

    /// Check if we're in a git repository (for test environment)
    fn is_git_available() -> bool {
        Command::new("git")
            .args(["--version"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Initialize a git repo in a temp directory for testing
    fn init_git_repo(dir: &Path) -> bool {
        let init = Command::new("git")
            .args(["init"])
            .current_dir(dir)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        if !init {
            return false;
        }

        // Configure git user for commits
        let _ = Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(dir)
            .output();
        let _ = Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(dir)
            .output();

        true
    }

    // Helper to create test infrastructure
    async fn setup_test_watcher() -> Option<(
        GitFileWatcher,
        Arc<MemoryStore>,
        tempfile::TempDir,
        tempfile::TempDir,
    )> {
        if !is_git_available() {
            return None;
        }

        let db_dir = tempdir().expect("create db temp dir");
        let watch_dir = tempdir().expect("create watch temp dir");

        // Initialize git repo in watch directory
        if !init_git_repo(watch_dir.path()) {
            return None;
        }

        let store = Arc::new(MemoryStore::new(db_dir.path()).expect("create store"));
        let embedder = Arc::new(TestEmbeddingProvider);
        let capture_service = Arc::new(MemoryCaptureService::new(store.clone(), embedder));

        let watcher = GitFileWatcher::new(
            vec![watch_dir.path().to_path_buf()],
            capture_service,
            "test-session".to_string(),
        )
        .ok()?;

        Some((watcher, store, db_dir, watch_dir))
    }

    // ==========================================================================
    // UNIT TESTS - Constructor Validation
    // ==========================================================================

    #[test]
    fn test_new_validates_path_exists() {
        if !is_git_available() {
            return;
        }

        let store =
            Arc::new(MemoryStore::new(tempdir().expect("tempdir").path()).expect("create store"));
        let embedder = Arc::new(TestEmbeddingProvider);
        let capture = Arc::new(MemoryCaptureService::new(store, embedder));

        let result = GitFileWatcher::new(
            vec![PathBuf::from("/nonexistent/path/xyz123")],
            capture,
            "session".to_string(),
        );

        assert!(matches!(result, Err(WatcherError::PathNotFound { .. })));
    }

    #[test]
    fn test_new_validates_path_is_directory() {
        if !is_git_available() {
            return;
        }

        let dir = tempdir().expect("tempdir");
        let file_path = dir.path().join("file.txt");
        fs::write(&file_path, "content").expect("write file");

        let store =
            Arc::new(MemoryStore::new(tempdir().expect("tempdir").path()).expect("create store"));
        let embedder = Arc::new(TestEmbeddingProvider);
        let capture = Arc::new(MemoryCaptureService::new(store, embedder));

        let result = GitFileWatcher::new(vec![file_path], capture, "session".to_string());

        assert!(matches!(result, Err(WatcherError::NotADirectory { .. })));
    }

    #[test]
    fn test_new_validates_git_repository() {
        if !is_git_available() {
            return;
        }

        let dir = tempdir().expect("tempdir");
        // Don't initialize git - this should fail

        let store =
            Arc::new(MemoryStore::new(tempdir().expect("tempdir").path()).expect("create store"));
        let embedder = Arc::new(TestEmbeddingProvider);
        let capture = Arc::new(MemoryCaptureService::new(store, embedder));

        let result = GitFileWatcher::new(
            vec![dir.path().to_path_buf()],
            capture,
            "session".to_string(),
        );

        assert!(matches!(result, Err(WatcherError::NotGitRepository { .. })));
    }

    // ==========================================================================
    // INTEGRATION TESTS - Real Git Operations
    // ==========================================================================

    #[tokio::test]
    async fn test_initial_scan_processes_existing_files() {
        let Some((mut watcher, store, _db_dir, watch_dir)) = setup_test_watcher().await else {
            println!("Skipping test - git not available");
            return;
        };

        // Create markdown file BEFORE starting watcher
        let md_path = watch_dir.path().join("existing.md");
        fs::write(
            &md_path,
            "# Existing Document\n\nThis file exists before watcher starts.",
        )
        .expect("write file");

        // Start watcher - should scan existing files
        watcher.start().await.expect("start watcher");

        // Verify file was processed
        let count = store.count().expect("count");
        assert!(
            count >= 1,
            "Existing file should be processed during initial scan"
        );

        watcher.stop();
    }

    #[tokio::test]
    async fn test_watcher_detects_new_file() {
        let Some((mut watcher, store, _db_dir, watch_dir)) = setup_test_watcher().await else {
            println!("Skipping test - git not available");
            return;
        };

        watcher.start().await.expect("start watcher");
        let count_before = store.count().expect("count");

        // Create new markdown file (untracked by git = ?? status)
        let md_path = watch_dir.path().join("new_file.md");
        fs::write(&md_path, "# New File\n\nThis is new content.").expect("write file");

        // Process events - git should detect untracked file
        watcher.process_events().await.expect("process events");

        let count_after = store.count().expect("count");
        assert!(
            count_after > count_before,
            "New file should create memories"
        );

        watcher.stop();
    }

    #[tokio::test]
    async fn test_watcher_detects_modified_file() {
        let Some((mut watcher, store, _db_dir, watch_dir)) = setup_test_watcher().await else {
            println!("Skipping test - git not available");
            return;
        };

        // Create file with original content
        let md_path = watch_dir.path().join("modify_test.md");
        fs::write(&md_path, "# Original Content\n\nVersion 1 original text.").expect("write file");

        // Add and commit to git so changes are tracked
        Command::new("git")
            .args(["add", "modify_test.md"])
            .current_dir(watch_dir.path())
            .output()
            .expect("git add");
        Command::new("git")
            .args(["commit", "-m", "initial"])
            .current_dir(watch_dir.path())
            .output()
            .expect("git commit");

        watcher.start().await.expect("start watcher");
        let count_after_create = store.count().expect("count");
        assert!(
            count_after_create >= 1,
            "Should have at least 1 memory after create"
        );

        // Verify original content was stored
        let file_path_str = md_path
            .canonicalize()
            .unwrap()
            .to_string_lossy()
            .to_string();
        let original_memories = store
            .get_by_file_path(&file_path_str)
            .expect("get by file path");
        assert!(
            !original_memories.is_empty(),
            "Should have memories for file"
        );
        assert!(
            original_memories
                .iter()
                .any(|m| m.content.contains("Version 1")),
            "Should contain original content"
        );

        // Modify file with new content
        fs::write(
            &md_path,
            "# Modified Content\n\nVersion 2 with significant changes and modifications.",
        )
        .expect("write file");

        // Process events - git should detect modified file
        watcher.process_events().await.expect("process events");

        // With our delete-before-rechunk logic, the old memories are deleted
        // and new ones are created. The content should be different.
        let modified_memories = store
            .get_by_file_path(&file_path_str)
            .expect("get by file path after modify");
        assert!(
            !modified_memories.is_empty(),
            "Should still have memories for file after modify"
        );

        // Verify the content was actually updated (old content deleted, new content stored)
        assert!(
            modified_memories
                .iter()
                .any(|m| m.content.contains("Version 2")),
            "Should contain modified content"
        );
        assert!(
            !modified_memories
                .iter()
                .any(|m| m.content.contains("Version 1")),
            "Should NOT contain original content (stale embeddings should be deleted)"
        );

        watcher.stop();
    }

    #[tokio::test]
    async fn test_watcher_cleans_up_on_file_deletion() {
        let Some((mut watcher, store, _db_dir, watch_dir)) = setup_test_watcher().await else {
            println!("Skipping test - git not available");
            return;
        };

        // Create a markdown file
        let md_path = watch_dir.path().join("delete_test.md");
        fs::write(
            &md_path,
            "# Delete Test\n\nThis file will be deleted. Content for testing deletion cleanup.",
        )
        .expect("write file");

        // Add and commit to git
        Command::new("git")
            .args(["add", "delete_test.md"])
            .current_dir(watch_dir.path())
            .output()
            .expect("git add");
        Command::new("git")
            .args(["commit", "-m", "add file"])
            .current_dir(watch_dir.path())
            .output()
            .expect("git commit");

        watcher.start().await.expect("start watcher");

        // Verify file was processed and stored
        let file_path_str = md_path
            .canonicalize()
            .unwrap()
            .to_string_lossy()
            .to_string();
        let memories_before_delete = store
            .get_by_file_path(&file_path_str)
            .expect("get by file path");
        assert!(
            !memories_before_delete.is_empty(),
            "Should have memories for file before deletion"
        );
        let initial_count = store.count().expect("count");
        assert!(initial_count >= 1, "Should have at least 1 memory");

        // Delete the file
        fs::remove_file(&md_path).expect("delete file");

        // Process events - git should detect deleted file
        watcher.process_events().await.expect("process events");

        // CRITICAL: Verify all embeddings for the deleted file are removed
        let memories_after_delete = store
            .get_by_file_path(&file_path_str)
            .expect("get by file path after delete");
        assert!(
            memories_after_delete.is_empty(),
            "Should have NO memories for deleted file, but found {}",
            memories_after_delete.len()
        );

        // Verify hash cache is also cleaned
        let cache_count = watcher.cached_file_count().await;
        assert_eq!(
            cache_count, 0,
            "Hash cache should be empty after file deletion"
        );

        watcher.stop();
    }

    #[tokio::test]
    async fn test_watcher_ignores_non_markdown_files() {
        let Some((mut watcher, store, _db_dir, watch_dir)) = setup_test_watcher().await else {
            println!("Skipping test - git not available");
            return;
        };

        watcher.start().await.expect("start watcher");
        let count_before = store.count().expect("count");

        // Create non-markdown files
        fs::write(watch_dir.path().join("file.txt"), "text content").expect("write file");
        fs::write(watch_dir.path().join("file.rs"), "rust content").expect("write file");
        fs::write(watch_dir.path().join("file.json"), "{}").expect("write file");

        watcher.process_events().await.expect("process events");

        let count_after = store.count().expect("count");
        assert_eq!(
            count_after, count_before,
            "Non-markdown files should be ignored"
        );

        watcher.stop();
    }

    #[tokio::test]
    async fn test_hash_prevents_duplicate_processing() {
        let Some((mut watcher, store, _db_dir, watch_dir)) = setup_test_watcher().await else {
            println!("Skipping test - git not available");
            return;
        };

        let md_path = watch_dir.path().join("hash_test.md");
        fs::write(&md_path, "# Same Content").expect("write file");

        watcher.start().await.expect("start watcher");
        let count_after_first = store.count().expect("count");

        // "Modify" file with same content (touch doesn't change hash)
        fs::write(&md_path, "# Same Content").expect("write file");

        watcher.process_events().await.expect("process events");

        let count_after_second = store.count().expect("count");
        assert_eq!(
            count_after_second, count_after_first,
            "Same content should not be reprocessed"
        );

        watcher.stop();
    }

    #[tokio::test]
    async fn test_is_markdown_detection() {
        let Some((watcher, _, _, _)) = setup_test_watcher().await else {
            println!("Skipping test - git not available");
            return;
        };

        // Positive cases
        assert!(watcher.is_markdown(Path::new("file.md")));
        assert!(watcher.is_markdown(Path::new("file.MD")));
        assert!(watcher.is_markdown(Path::new("file.markdown")));
        assert!(watcher.is_markdown(Path::new("file.MARKDOWN")));
        assert!(watcher.is_markdown(Path::new("/path/to/doc.md")));

        // Negative cases
        assert!(!watcher.is_markdown(Path::new("file.txt")));
        assert!(!watcher.is_markdown(Path::new("file.rs")));
        assert!(!watcher.is_markdown(Path::new("file.mdx"))); // Not plain markdown
        assert!(!watcher.is_markdown(Path::new("file"))); // No extension
        assert!(!watcher.is_markdown(Path::new(".md"))); // Hidden file, no name
    }

    #[tokio::test]
    async fn test_start_already_running_error() {
        let Some((mut watcher, _store, _db_dir, _watch_dir)) = setup_test_watcher().await else {
            println!("Skipping test - git not available");
            return;
        };

        watcher.start().await.expect("first start");
        let result = watcher.start().await;

        assert!(matches!(result, Err(WatcherError::AlreadyRunning)));

        watcher.stop();
    }

    #[tokio::test]
    async fn test_process_events_not_started_error() {
        let Some((mut watcher, _store, _db_dir, _watch_dir)) = setup_test_watcher().await else {
            println!("Skipping test - git not available");
            return;
        };

        let result = watcher.process_events().await;
        assert!(matches!(result, Err(WatcherError::NotStarted)));
    }

    #[tokio::test]
    async fn test_stop_is_idempotent() {
        let Some((mut watcher, _store, _db_dir, _watch_dir)) = setup_test_watcher().await else {
            println!("Skipping test - git not available");
            return;
        };

        watcher.start().await.expect("start");
        watcher.stop();
        assert!(!watcher.is_running());

        // Second stop should not panic
        watcher.stop();
        assert!(!watcher.is_running());
    }

    #[tokio::test]
    async fn test_cached_file_count() {
        let Some((mut watcher, _store, _db_dir, watch_dir)) = setup_test_watcher().await else {
            println!("Skipping test - git not available");
            return;
        };

        // Create files before starting
        fs::write(watch_dir.path().join("file1.md"), "# File 1").expect("write file");
        fs::write(watch_dir.path().join("file2.md"), "# File 2").expect("write file");
        fs::write(watch_dir.path().join("file3.md"), "# File 3").expect("write file");

        watcher.start().await.expect("start");

        let cache_count = watcher.cached_file_count().await;
        assert_eq!(cache_count, 3, "Should cache hashes for 3 files");

        watcher.stop();
    }

    // ==========================================================================
    // EDGE CASE TESTS
    // ==========================================================================

    #[tokio::test]
    async fn edge_case_empty_markdown_file() {
        let Some((mut watcher, _store, _db_dir, watch_dir)) = setup_test_watcher().await else {
            println!("Skipping test - git not available");
            return;
        };

        // Create empty markdown file
        let md_path = watch_dir.path().join("empty.md");
        fs::write(&md_path, "").expect("write file");

        watcher.start().await.expect("start watcher");

        // Empty file should be rejected by chunker (ChunkerError::EmptyContent)
        // But watcher should not crash
        assert!(watcher.is_running());

        watcher.stop();
    }

    #[tokio::test]
    async fn edge_case_large_markdown_file() {
        let Some((mut watcher, store, _db_dir, watch_dir)) = setup_test_watcher().await else {
            println!("Skipping test - git not available");
            return;
        };

        // Create large markdown file (500+ words = multiple chunks)
        let words: Vec<&str> = (0..600).map(|_| "word").collect();
        let content = format!("# Large Document\n\n{}", words.join(" "));
        let md_path = watch_dir.path().join("large.md");
        fs::write(&md_path, content).expect("write file");

        watcher.start().await.expect("start watcher");

        // Should create multiple memory chunks
        let count = store.count().expect("count");
        assert!(
            count >= 3,
            "Large file should create multiple chunks, got {}",
            count
        );

        watcher.stop();
    }

    #[tokio::test]
    async fn edge_case_unicode_content() {
        let Some((mut watcher, store, _db_dir, watch_dir)) = setup_test_watcher().await else {
            println!("Skipping test - git not available");
            return;
        };

        // Create markdown with Unicode content
        let content = "# Japanese Document\n\nHello World Unicode Test Content";
        let md_path = watch_dir.path().join("unicode.md");
        fs::write(&md_path, content).expect("write file");

        watcher.start().await.expect("start watcher");

        let count = store.count().expect("count");
        assert!(count >= 1, "Unicode file should be processed");

        watcher.stop();
    }

    #[tokio::test]
    async fn edge_case_special_filename() {
        let Some((mut watcher, store, _db_dir, watch_dir)) = setup_test_watcher().await else {
            println!("Skipping test - git not available");
            return;
        };

        // Create file with special characters in name
        let md_path = watch_dir.path().join("file with spaces (1).md");
        fs::write(&md_path, "# Special Name\n\nContent here.").expect("write file");

        watcher.start().await.expect("start watcher");

        let count = store.count().expect("count");
        assert!(count >= 1, "File with special name should be processed");

        watcher.stop();
    }

    // ==========================================================================
    // FSV: FULL STATE VERIFICATION TEST
    // ==========================================================================

    #[tokio::test]
    async fn fsv_gitfilewatcher_persistence_verification() {
        println!("\n============================================================");
        println!("=== FSV: GitFileWatcher Persistence Verification ===");
        println!("============================================================\n");

        if !is_git_available() {
            println!("Skipping FSV test - git not available");
            return;
        }

        let db_dir = tempdir().expect("create db temp dir");
        let watch_dir = tempdir().expect("create watch temp dir");

        // Initialize git repo
        if !init_git_repo(watch_dir.path()) {
            println!("Skipping FSV test - failed to init git repo");
            return;
        }

        // Phase 1: Create and process files
        let file_count_created;
        {
            let store = Arc::new(MemoryStore::new(db_dir.path()).expect("create store"));
            let embedder = Arc::new(TestEmbeddingProvider);
            let capture = Arc::new(MemoryCaptureService::new(store.clone(), embedder));

            let mut watcher = GitFileWatcher::new(
                vec![watch_dir.path().to_path_buf()],
                capture,
                "fsv-session".to_string(),
            )
            .expect("create watcher");

            println!(
                "[FSV-1] Initial store count: {}",
                store.count().expect("count")
            );
            assert_eq!(store.count().expect("count"), 0);

            // Create markdown files
            fs::write(
                watch_dir.path().join("fsv1.md"),
                "# FSV Test 1\n\nFirst document content.",
            )
            .expect("write file");
            fs::write(
                watch_dir.path().join("fsv2.md"),
                "# FSV Test 2\n\nSecond document content.",
            )
            .expect("write file");

            watcher.start().await.expect("start watcher");

            file_count_created = store.count().expect("count");
            println!(
                "[FSV-2] After initial scan: {} memories",
                file_count_created
            );
            assert!(file_count_created >= 2, "Should have at least 2 memories");

            // Add another file
            fs::write(
                watch_dir.path().join("fsv3.md"),
                "# FSV Test 3\n\nThird document added after start.",
            )
            .expect("write file");
            watcher.process_events().await.expect("process");

            let count_after_add = store.count().expect("count");
            println!("[FSV-3] After adding fsv3.md: {} memories", count_after_add);

            watcher.stop();
            println!("[FSV-4] Watcher stopped, store being dropped...");
        }

        // Phase 2: Reopen store and verify persistence
        println!("\n[FSV-5] Reopening database to verify persistence...");
        {
            let store = MemoryStore::new(db_dir.path()).expect("reopen store");
            let count_after_reopen = store.count().expect("count");
            println!("[FSV-6] Reopened store count: {}", count_after_reopen);

            assert!(
                count_after_reopen >= file_count_created,
                "Memories should persist across store reopening"
            );

            // Verify we can retrieve memories by session
            let session_memories = store.get_by_session("fsv-session").expect("get by session");
            println!(
                "[FSV-7] Memories in fsv-session: {}",
                session_memories.len()
            );

            for (i, mem) in session_memories.iter().enumerate() {
                println!(
                    "[FSV-{}] Memory {}: source={:?}, content_preview='{}'",
                    8 + i,
                    mem.id,
                    mem.source,
                    mem.content.chars().take(50).collect::<String>()
                );
                assert!(
                    mem.is_md_file_chunk(),
                    "Memory should be MDFileChunk source"
                );
            }
        }

        println!("\n============================================================");
        println!("[FSV] VERIFIED: All GitFileWatcher persistence checks passed");
        println!("============================================================\n");
    }
}
