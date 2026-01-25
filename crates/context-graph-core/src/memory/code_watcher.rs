//! CodeFileWatcher - Git-based file watcher for code file changes.
//!
//! Monitors directories for code file changes and automatically parses them
//! using tree-sitter AST chunking, then stores code entities with E7 embeddings.
//!
//! # Architecture
//! Git Commands -> File Read -> ASTChunker -> CodeCaptureService -> CodeStore
//!
//! # Supported Languages
//! Currently: Rust (.rs)
//! Future: Python, TypeScript, JavaScript, Go
//!
//! # Constitution Compliance
//! - E7 (V_correctness): 1536D code patterns, function signatures
//! - Code embeddings are stored separately from the 13-embedder teleological system

use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;
use tracing::{debug, error, info, instrument, warn};

use super::ast_chunker::{AstChunkConfig, AstChunkerError, AstCodeChunker};
use super::code_capture::{CodeCaptureError, CodeCaptureService, CodeEmbeddingProvider, CodeStorage};
use crate::types::CodeLanguage;

/// Errors from CodeFileWatcher operations.
#[derive(Debug, Error)]
pub enum CodeWatcherError {
    /// Path does not exist or is not accessible.
    #[error("Path not found: {path:?}")]
    PathNotFound { path: PathBuf },

    /// Path is not a directory.
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

    /// AST chunking failed.
    #[error("AST chunking failed for {path:?}: {source}")]
    ChunkingFailed {
        path: PathBuf,
        #[source]
        source: AstChunkerError,
    },

    /// Code capture failed.
    #[error("Capture failed for {path:?}: {source}")]
    CaptureFailed {
        path: PathBuf,
        #[source]
        source: CodeCaptureError,
    },

    /// Watcher is not started.
    #[error("Watcher not started - call start() first")]
    NotStarted,

    /// Watcher is already running.
    #[error("Watcher already running")]
    AlreadyRunning,

    /// Unsupported language.
    #[error("Unsupported language: {language}")]
    UnsupportedLanguage { language: String },
}

/// Git change status.
#[derive(Debug, Clone)]
struct GitChange {
    /// Git status code (e.g., "M", " M", "??").
    status: String,
    /// Relative path to the file.
    path: String,
}

/// CodeFileWatcher monitors directories for code file changes using git.
///
/// # Architecture
/// - Uses `git ls-files` for initial file discovery
/// - Uses `git status --porcelain` for change detection
/// - Tracks SHA256 hashes to detect actual content changes
/// - Processes files through ASTChunker (tree-sitter)
/// - Stores chunks via CodeCaptureService with E7 embeddings
///
/// # Thread Safety
/// The watcher is `Send + Sync` and can be shared across async tasks.
pub struct CodeFileWatcher<E: CodeEmbeddingProvider, S: CodeStorage> {
    /// Code capture service for storing entities.
    capture_service: Arc<CodeCaptureService<E, S>>,

    /// AST chunker for parsing code.
    chunker: AstCodeChunker,

    /// File content hashes for change detection.
    file_hashes: Arc<RwLock<HashMap<PathBuf, String>>>,

    /// Session ID for captured entities.
    #[allow(dead_code)]
    session_id: String,

    /// Paths being watched.
    watch_paths: Vec<PathBuf>,

    /// Supported file extensions.
    supported_extensions: HashSet<String>,

    /// Running state flag.
    is_running: bool,

    /// Git repository root.
    git_root: Option<PathBuf>,
}

impl<E: CodeEmbeddingProvider, S: CodeStorage> CodeFileWatcher<E, S> {
    /// Create a new CodeFileWatcher.
    ///
    /// # Arguments
    /// * `watch_paths` - Directories to watch for code files
    /// * `capture_service` - Service for capturing code entities
    /// * `session_id` - Session to associate entities with
    pub fn new(
        watch_paths: Vec<PathBuf>,
        capture_service: Arc<CodeCaptureService<E, S>>,
        session_id: String,
    ) -> Result<Self, CodeWatcherError> {
        // Check git is available
        Self::check_git_available()?;

        // Validate all paths exist and are directories
        for path in &watch_paths {
            if !path.exists() {
                return Err(CodeWatcherError::PathNotFound { path: path.clone() });
            }
            if !path.is_dir() {
                return Err(CodeWatcherError::NotADirectory { path: path.clone() });
            }
        }

        // Detect git root from first watch path
        let git_root = if !watch_paths.is_empty() {
            Some(Self::detect_git_root(&watch_paths[0])?)
        } else {
            None
        };

        // Create AST chunker for Rust
        let chunker = AstCodeChunker::default_rust().map_err(|e| CodeWatcherError::ChunkingFailed {
            path: PathBuf::from("init"),
            source: e,
        })?;

        // Supported extensions
        let mut supported_extensions = HashSet::new();
        supported_extensions.insert("rs".to_string());
        // Future: Add "py", "ts", "js", "go"

        Ok(Self {
            capture_service,
            chunker,
            file_hashes: Arc::new(RwLock::new(HashMap::new())),
            session_id,
            watch_paths,
            supported_extensions,
            is_running: false,
            git_root,
        })
    }

    /// Create with custom AST chunk configuration.
    pub fn with_config(
        watch_paths: Vec<PathBuf>,
        capture_service: Arc<CodeCaptureService<E, S>>,
        session_id: String,
        config: AstChunkConfig,
    ) -> Result<Self, CodeWatcherError> {
        let mut watcher = Self::new(watch_paths, capture_service, session_id)?;
        watcher.chunker = AstCodeChunker::new_rust(config).map_err(|e| CodeWatcherError::ChunkingFailed {
            path: PathBuf::from("init"),
            source: e,
        })?;
        Ok(watcher)
    }

    /// Check if git is available in PATH.
    fn check_git_available() -> Result<(), CodeWatcherError> {
        let output = Command::new("git").arg("--version").output();

        match output {
            Ok(output) if output.status.success() => Ok(()),
            Ok(output) => Err(CodeWatcherError::GitCommandFailed {
                command: "git --version".to_string(),
                message: String::from_utf8_lossy(&output.stderr).to_string(),
            }),
            Err(_) => Err(CodeWatcherError::GitNotFound),
        }
    }

    /// Detect the git repository root for a path.
    fn detect_git_root(path: &Path) -> Result<PathBuf, CodeWatcherError> {
        let output = Command::new("git")
            .args(["rev-parse", "--show-toplevel"])
            .current_dir(path)
            .output()
            .map_err(|_| CodeWatcherError::GitNotFound)?;

        if output.status.success() {
            let root = String::from_utf8_lossy(&output.stdout).trim().to_string();
            Ok(PathBuf::from(root))
        } else {
            Err(CodeWatcherError::NotGitRepository {
                path: path.to_path_buf(),
            })
        }
    }

    /// Start watching directories and perform initial scan.
    #[instrument(skip(self))]
    pub async fn start(&mut self) -> Result<(), CodeWatcherError> {
        if self.is_running {
            return Err(CodeWatcherError::AlreadyRunning);
        }

        self.is_running = true;

        // Perform initial scan using git ls-files
        for path in &self.watch_paths.clone() {
            if let Err(e) = self.scan_directory_git(path).await {
                warn!(path = ?path, error = %e, "Initial scan failed for directory");
            }
        }

        info!(
            paths = ?self.watch_paths,
            session_id = %self.session_id,
            git_root = ?self.git_root,
            extensions = ?self.supported_extensions,
            "CodeFileWatcher started"
        );

        Ok(())
    }

    /// Stop watching.
    pub fn stop(&mut self) {
        self.is_running = false;
        info!("CodeFileWatcher stopped");
    }

    /// Check if the watcher is running.
    pub fn is_running(&self) -> bool {
        self.is_running
    }

    /// Process pending changes from git status.
    ///
    /// Call this method periodically to detect and process file changes.
    #[instrument(skip(self))]
    pub async fn process_events(&mut self) -> Result<usize, CodeWatcherError> {
        if !self.is_running {
            return Err(CodeWatcherError::NotStarted);
        }

        let mut files_processed = 0;

        for path in &self.watch_paths.clone() {
            let changes = self.get_git_changes(path)?;

            for change in changes {
                match change.status.as_str() {
                    // Deleted files
                    "D " | " D" | "DD" | "AD" => {
                        let full_path = self.resolve_path(&change.path)?;
                        match self.handle_file_deletion(&full_path).await {
                            Ok(deleted) => {
                                if deleted > 0 {
                                    files_processed += 1;
                                    info!(
                                        path = ?change.path,
                                        deleted_count = deleted,
                                        "Cleaned up code entities for deleted file"
                                    );
                                }
                            }
                            Err(e) => {
                                error!(
                                    path = ?change.path,
                                    error = %e,
                                    "Failed to clean up code entities for deleted file"
                                );
                            }
                        }
                    }
                    // Modified or new files
                    "M" | " M" | "??" | "A" | " A" | "AM" | "MM" => {
                        let full_path = self.resolve_path(&change.path)?;
                        if self.is_supported_code_file(&full_path) && full_path.exists() {
                            match self.process_file(&full_path).await {
                                Ok(ids) => {
                                    if !ids.is_empty() {
                                        files_processed += 1;
                                        info!(
                                            path = ?change.path,
                                            entities = ids.len(),
                                            "Processed code file"
                                        );
                                    }
                                }
                                Err(e) => {
                                    error!(path = ?change.path, error = %e, "Failed to process code file");
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
    fn get_git_changes(&self, path: &Path) -> Result<Vec<GitChange>, CodeWatcherError> {
        let output = Command::new("git")
            .args(["status", "--porcelain", "."])
            .current_dir(path)
            .output()
            .map_err(|e| CodeWatcherError::GitCommandFailed {
                command: "git status --porcelain".to_string(),
                message: e.to_string(),
            })?;

        if !output.status.success() {
            return Err(CodeWatcherError::GitCommandFailed {
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
                let path = line[3..].to_string();
                Some(GitChange { status, path })
            })
            .collect();

        Ok(changes)
    }

    /// Scan a directory using git ls-files.
    async fn scan_directory_git(&mut self, dir: &Path) -> Result<usize, CodeWatcherError> {
        let output = Command::new("git")
            .args(["ls-files", "--cached", "--others", "--exclude-standard", "."])
            .current_dir(dir)
            .output()
            .map_err(|e| CodeWatcherError::GitCommandFailed {
                command: "git ls-files".to_string(),
                message: e.to_string(),
            })?;

        if !output.status.success() {
            return Err(CodeWatcherError::GitCommandFailed {
                command: "git ls-files".to_string(),
                message: String::from_utf8_lossy(&output.stderr).to_string(),
            });
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut processed = 0;

        for relative_path in stdout.lines() {
            let full_path = dir.join(relative_path);

            if self.is_supported_code_file(&full_path) && full_path.exists() {
                match self.process_file(&full_path).await {
                    Ok(ids) => {
                        if !ids.is_empty() {
                            processed += 1;
                            debug!(path = ?relative_path, entities = ids.len(), "Processed code file");
                        }
                    }
                    Err(e) => {
                        warn!(path = ?relative_path, error = %e, "Failed to process code file");
                    }
                }
            }
        }

        info!(dir = ?dir, processed = processed, "Completed directory scan");
        Ok(processed)
    }

    /// Process a single code file.
    async fn process_file(&mut self, path: &Path) -> Result<Vec<uuid::Uuid>, CodeWatcherError> {
        // Read file content
        let content = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| CodeWatcherError::ReadFailed {
                path: path.to_path_buf(),
                source: e,
            })?;

        // Compute hash
        let hash = Self::compute_hash(&content);

        // Check if content has changed
        {
            let hashes = self.file_hashes.read().await;
            if let Some(existing_hash) = hashes.get(path) {
                if existing_hash == &hash {
                    debug!(path = ?path, "File unchanged, skipping");
                    return Ok(Vec::new());
                }
            }
        }

        // Delete existing entities for this file
        let path_str = path.to_string_lossy().to_string();
        self.capture_service
            .delete_by_file(&path_str)
            .await
            .map_err(|e| CodeWatcherError::CaptureFailed {
                path: path.to_path_buf(),
                source: e,
            })?;

        // Parse file into chunks
        let chunks = self
            .chunker
            .chunk(&content, &path_str)
            .map_err(|e| CodeWatcherError::ChunkingFailed {
                path: path.to_path_buf(),
                source: e,
            })?;

        if chunks.is_empty() {
            debug!(path = ?path, "No code entities found in file");
            // Update hash to prevent re-processing
            let mut hashes = self.file_hashes.write().await;
            hashes.insert(path.to_path_buf(), hash);
            return Ok(Vec::new());
        }

        // Capture all chunks
        let ids = self
            .capture_service
            .capture_batch(chunks)
            .await
            .map_err(|e| CodeWatcherError::CaptureFailed {
                path: path.to_path_buf(),
                source: e,
            })?;

        // Update hash
        let mut hashes = self.file_hashes.write().await;
        hashes.insert(path.to_path_buf(), hash);

        Ok(ids)
    }

    /// Handle file deletion by removing entities.
    async fn handle_file_deletion(&self, path: &Path) -> Result<usize, CodeWatcherError> {
        let path_str = path.to_string_lossy().to_string();
        self.capture_service
            .delete_by_file(&path_str)
            .await
            .map_err(|e| CodeWatcherError::CaptureFailed {
                path: path.to_path_buf(),
                source: e,
            })
    }

    /// Check if a file has a supported code extension.
    fn is_supported_code_file(&self, path: &Path) -> bool {
        path.extension()
            .and_then(|e| e.to_str())
            .map(|ext| self.supported_extensions.contains(ext))
            .unwrap_or(false)
    }

    /// Resolve a relative path to an absolute path.
    fn resolve_path(&self, relative: &str) -> Result<PathBuf, CodeWatcherError> {
        if let Some(ref root) = self.git_root {
            Ok(root.join(relative))
        } else if !self.watch_paths.is_empty() {
            Ok(self.watch_paths[0].join(relative))
        } else {
            Ok(PathBuf::from(relative))
        }
    }

    /// Compute SHA256 hash of content.
    fn compute_hash(content: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// Detect language from file extension.
    #[allow(dead_code)]
    fn detect_language(path: &Path) -> CodeLanguage {
        CodeLanguage::from_path(&path.to_string_lossy())
    }

    /// Get statistics about watched files.
    pub async fn stats(&self) -> WatcherStats {
        let hashes = self.file_hashes.read().await;
        WatcherStats {
            files_tracked: hashes.len(),
            is_running: self.is_running,
            watch_paths: self.watch_paths.clone(),
            supported_extensions: self.supported_extensions.clone(),
        }
    }
}

/// Statistics about the code file watcher.
#[derive(Debug, Clone)]
pub struct WatcherStats {
    /// Number of files currently tracked.
    pub files_tracked: usize,
    /// Whether the watcher is running.
    pub is_running: bool,
    /// Paths being watched.
    pub watch_paths: Vec<PathBuf>,
    /// Supported file extensions.
    pub supported_extensions: HashSet<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::collections::HashMap as StdHashMap;
    use tokio::sync::RwLock as TokioRwLock;
    use uuid::Uuid;

    use super::super::code_capture::{CodeEmbedderError, CodeEmbeddingProvider, CodeStorage};
    use crate::types::fingerprint::SemanticFingerprint;
    use crate::types::CodeEntity;

    /// Mock embedding provider for testing.
    struct MockCodeEmbedder;

    #[async_trait]
    impl CodeEmbeddingProvider for MockCodeEmbedder {
        async fn embed_code(&self, _code: &str, _context: Option<&str>) -> Result<SemanticFingerprint, CodeEmbedderError> {
            // Return a complete zeroed fingerprint (all 13 embeddings)
            Ok(SemanticFingerprint::zeroed())
        }

        async fn embed_batch(&self, codes: &[(&str, Option<&str>)]) -> Result<Vec<SemanticFingerprint>, CodeEmbedderError> {
            Ok(codes.iter().map(|_| SemanticFingerprint::zeroed()).collect())
        }

        fn is_ready(&self) -> bool {
            true
        }
    }

    /// Mock storage for testing.
    struct MockCodeStorage {
        entities: TokioRwLock<StdHashMap<Uuid, (CodeEntity, SemanticFingerprint)>>,
        file_index: TokioRwLock<StdHashMap<String, Vec<Uuid>>>,
    }

    impl MockCodeStorage {
        fn new() -> Self {
            Self {
                entities: TokioRwLock::new(StdHashMap::new()),
                file_index: TokioRwLock::new(StdHashMap::new()),
            }
        }
    }

    #[async_trait]
    impl CodeStorage for MockCodeStorage {
        async fn store(&self, entity: &CodeEntity, fingerprint: &SemanticFingerprint) -> Result<(), String> {
            let mut entities = self.entities.write().await;
            let mut file_index = self.file_index.write().await;

            entities.insert(entity.id, (entity.clone(), fingerprint.clone()));
            file_index
                .entry(entity.file_path.clone())
                .or_default()
                .push(entity.id);

            Ok(())
        }

        async fn get(&self, id: Uuid) -> Result<Option<CodeEntity>, String> {
            let entities = self.entities.read().await;
            Ok(entities.get(&id).map(|(e, _)| e.clone()))
        }

        async fn get_by_file(&self, file_path: &str) -> Result<Vec<CodeEntity>, String> {
            let file_index = self.file_index.read().await;
            let entities = self.entities.read().await;

            let ids = file_index.get(file_path).cloned().unwrap_or_default();
            Ok(ids
                .iter()
                .filter_map(|id| entities.get(id).map(|(e, _)| e.clone()))
                .collect())
        }

        async fn delete_file(&self, file_path: &str) -> Result<usize, String> {
            let mut file_index = self.file_index.write().await;
            let mut entities = self.entities.write().await;

            let ids = file_index.remove(file_path).unwrap_or_default();
            let count = ids.len();

            for id in ids {
                entities.remove(&id);
            }

            Ok(count)
        }

        async fn get_fingerprint(&self, id: Uuid) -> Result<Option<SemanticFingerprint>, String> {
            let entities = self.entities.read().await;
            Ok(entities.get(&id).map(|(_, fp)| fp.clone()))
        }

        async fn search_by_fingerprint(
            &self,
            _query_fingerprint: &SemanticFingerprint,
            _top_k: usize,
            _min_similarity: f32,
            _use_e7_primary: bool,
        ) -> Result<Vec<(CodeEntity, f32)>, String> {
            // Mock implementation - returns empty for tests
            Ok(Vec::new())
        }
    }

    #[test]
    fn test_is_supported_code_file() {
        let embedder = Arc::new(MockCodeEmbedder);
        let storage = Arc::new(MockCodeStorage::new());
        let capture = Arc::new(CodeCaptureService::new(embedder, storage, "test".to_string()));

        // Create watcher in current directory (likely has git)
        let watcher_result = CodeFileWatcher::new(
            vec![PathBuf::from(".")],
            capture,
            "test".to_string(),
        );

        // This test may fail if not in a git repo, which is fine
        if let Ok(watcher) = watcher_result {
            assert!(watcher.is_supported_code_file(Path::new("test.rs")));
            assert!(!watcher.is_supported_code_file(Path::new("test.txt")));
            assert!(!watcher.is_supported_code_file(Path::new("test.md")));
        }
    }

    #[test]
    fn test_compute_hash() {
        let hash1 = CodeFileWatcher::<MockCodeEmbedder, MockCodeStorage>::compute_hash("fn main() {}");
        let hash2 = CodeFileWatcher::<MockCodeEmbedder, MockCodeStorage>::compute_hash("fn main() {}");
        let hash3 = CodeFileWatcher::<MockCodeEmbedder, MockCodeStorage>::compute_hash("fn other() {}");

        assert_eq!(hash1, hash2, "Same content should produce same hash");
        assert_ne!(hash1, hash3, "Different content should produce different hash");
        assert_eq!(hash1.len(), 64, "SHA256 hash should be 64 hex chars");
    }

    #[test]
    fn test_detect_language() {
        assert_eq!(
            CodeFileWatcher::<MockCodeEmbedder, MockCodeStorage>::detect_language(Path::new("test.rs")),
            CodeLanguage::Rust
        );
        assert_eq!(
            CodeFileWatcher::<MockCodeEmbedder, MockCodeStorage>::detect_language(Path::new("test.py")),
            CodeLanguage::Python
        );
        assert_eq!(
            CodeFileWatcher::<MockCodeEmbedder, MockCodeStorage>::detect_language(Path::new("test.unknown")),
            CodeLanguage::Unknown
        );
    }
}
