use context_graph_core::memory::ast::{self, AstChunk, AstChunkOptions};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use thiserror::Error;

const CACHE_SCHEMA_VERSION: u32 = 1;
const WATCHER_STATUS_FILE: &str = "watcher.status";
pub const PROJECT_CACHE_DEBOUNCE_MS: u64 = 250;

#[derive(Debug, Error)]
pub enum ProjectCacheError {
    #[error("MEJEPA_PROJECT_CACHE_INVALID_PATH: {path} must be an existing git repository")]
    InvalidRepoPath { path: String },
    #[error(
        "MEJEPA_PROJECT_CACHE_ROOT_NOT_AIWONDER: {path} must live under /zfs/archive/contextgraph/projects in production"
    )]
    InvalidCacheRoot { path: String },
    #[error("{0}")]
    Path(#[from] context_graph_paths::PathError),
    #[error("MEJEPA_PROJECT_CACHE_IO: {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("MEJEPA_PROJECT_CACHE_SQLITE: {context}: {source}")]
    Sqlite {
        context: &'static str,
        #[source]
        source: rusqlite::Error,
    },
    #[error("MEJEPA_PROJECT_CACHE_GIT_FAILED: {stderr}")]
    GitFailed { stderr: String },
    #[error("MEJEPA_PROJECT_CACHE_UNSUPPORTED_FILE: {path}")]
    UnsupportedFile { path: String },
    #[error("MEJEPA_PROJECT_CACHE_CHUNK_FAILED: {path}: {reason}")]
    ChunkFailed { path: String, reason: String },
    #[error("MEJEPA_PROJECT_CACHE_HASH_COLLISION: blake3={blake3} paths={paths:?}")]
    HashCollision { blake3: String, paths: Vec<String> },
}

impl ProjectCacheError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidRepoPath { .. } => "MEJEPA_PROJECT_CACHE_INVALID_PATH",
            Self::InvalidCacheRoot { .. } => "MEJEPA_PROJECT_CACHE_ROOT_NOT_AIWONDER",
            Self::Path(err) => err.code,
            Self::Io { .. } => "MEJEPA_PROJECT_CACHE_IO",
            Self::Sqlite { .. } => "MEJEPA_PROJECT_CACHE_SQLITE",
            Self::GitFailed { .. } => "MEJEPA_PROJECT_CACHE_GIT_FAILED",
            Self::UnsupportedFile { .. } => "MEJEPA_PROJECT_CACHE_UNSUPPORTED_FILE",
            Self::ChunkFailed { .. } => "MEJEPA_PROJECT_CACHE_CHUNK_FAILED",
            Self::HashCollision { .. } => "MEJEPA_PROJECT_CACHE_HASH_COLLISION",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProjectCacheConfig {
    pub project_id: String,
    pub repo_path: PathBuf,
    pub cache_dir: PathBuf,
    pub chunk_schema_version: String,
    pub scope: ProjectFileScope,
    pub single_file: Option<String>,
}

impl ProjectCacheConfig {
    pub fn new(project_id: impl Into<String>, repo_path: PathBuf) -> Self {
        let project_id = project_id.into();
        Self {
            cache_dir: PathBuf::from(context_graph_paths::DEFAULT_DATA_ROOT)
                .join("projects")
                .join(&project_id)
                .join("cache"),
            project_id,
            repo_path,
            chunk_schema_version: "python-ast-v1".to_string(),
            scope: ProjectFileScope::All,
            single_file: None,
        }
    }

    pub fn with_scope(mut self, scope: ProjectFileScope) -> Self {
        self.scope = scope;
        self
    }

    pub fn with_single_file(mut self, rel_path: Option<String>) -> Self {
        self.single_file = rel_path;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChunkRole {
    Source,
    Test,
    Doc,
    Config,
}

impl ChunkRole {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Source => "source",
            Self::Test => "test",
            Self::Doc => "doc",
            Self::Config => "config",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum ProjectFileScope {
    SourceOnly,
    SourceAndTests,
    All,
}

impl ProjectFileScope {
    pub fn includes_path(self, path: &str) -> bool {
        match self {
            Self::SourceOnly => project_role_for_path(path) == ChunkRole::Source,
            Self::SourceAndTests => {
                matches!(
                    project_role_for_path(path),
                    ChunkRole::Source | ChunkRole::Test
                )
            }
            Self::All => true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct EmbedderVersion {
    pub embedder_id: String,
    pub embedder_version: String,
}

impl EmbedderVersion {
    pub fn new(embedder_id: impl Into<String>, embedder_version: impl Into<String>) -> Self {
        Self {
            embedder_id: embedder_id.into(),
            embedder_version: embedder_version.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectCacheReport {
    pub project_id: String,
    pub repo_path: String,
    pub db_path: String,
    pub watcher_status_path: String,
    pub merkle_root: String,
    pub scan_started_unix_ms: i64,
    pub file_count: usize,
    pub chunk_path_count: usize,
    pub unique_chunk_count: usize,
    pub embedding_count: usize,
    pub changed_files: Vec<String>,
    pub deleted_files: Vec<String>,
    pub deduped_chunk_paths: usize,
    pub lazy_rescan_queued: usize,
    pub per_embedder_reembedded: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatcherStatus {
    pub project_id: String,
    pub heartbeat_unix_ms: i64,
    pub lag_count: u64,
    pub file_count: usize,
    pub merkle_root: String,
}

#[derive(Debug)]
pub struct ProjectMerkleCache {
    config: ProjectCacheConfig,
    db_path: PathBuf,
    status_path: PathBuf,
}

impl ProjectMerkleCache {
    pub fn open(config: ProjectCacheConfig) -> Result<Self, ProjectCacheError> {
        let repo_path = canonicalize_existing_dir(&config.repo_path).map_err(|source| {
            ProjectCacheError::Io {
                path: config.repo_path.display().to_string(),
                source,
            }
        })?;
        let cache_dir = require_project_cache_root(&config.cache_dir)?;
        fs::create_dir_all(&cache_dir).map_err(|source| ProjectCacheError::Io {
            path: cache_dir.display().to_string(),
            source,
        })?;
        let db_path = cache_dir.join("cache.db");
        let status_path = cache_dir.join(WATCHER_STATUS_FILE);
        let cache = Self {
            config: ProjectCacheConfig {
                repo_path,
                cache_dir,
                ..config
            },
            db_path,
            status_path,
        };
        cache.init_schema()?;
        Ok(cache)
    }

    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    pub fn watcher_status_path(&self) -> &Path {
        &self.status_path
    }

    pub fn scan_project(
        &self,
        embedder_versions: &[EmbedderVersion],
    ) -> Result<ProjectCacheReport, ProjectCacheError> {
        let scan_started_unix_ms = now_ms();
        let conn = self.open_connection()?;
        let previous = load_existing_files(&conn)?;
        let tracked = project_files(&self.config.repo_path)?
            .into_iter()
            .filter(|path| {
                self.config
                    .single_file
                    .as_ref()
                    .map(|only| only == path)
                    .unwrap_or(true)
            })
            .filter(|path| self.config.scope.includes_path(path))
            .collect::<Vec<_>>();
        let tracked_set: BTreeSet<String> = tracked.iter().cloned().collect();
        let deleted_files: Vec<String> = previous
            .keys()
            .filter(|path| !tracked_set.contains(*path))
            .cloned()
            .collect();
        let mut changed_files = Vec::new();
        let mut files_by_hash: BTreeMap<String, Vec<(String, u64)>> = BTreeMap::new();
        let mut chunks_to_refresh = BTreeSet::new();

        for rel_path in &tracked {
            let absolute = self.config.repo_path.join(rel_path);
            let metadata = fs::metadata(&absolute).map_err(|source| ProjectCacheError::Io {
                path: absolute.display().to_string(),
                source,
            })?;
            let size_bytes = metadata.len();
            let mtime_ns = metadata_modified_ns(&metadata);
            let previous_row = previous.get(rel_path);
            let metadata_unchanged = previous_row.is_some_and(|old| {
                old.size_bytes == size_bytes && old.mtime_ns != 0 && old.mtime_ns == mtime_ns
            });
            let (file_hash, bytes) = if metadata_unchanged {
                (
                    previous_row
                        .map(|old| old.blake3.clone())
                        .unwrap_or_default(),
                    None,
                )
            } else {
                let bytes = fs::read(&absolute).map_err(|source| ProjectCacheError::Io {
                    path: absolute.display().to_string(),
                    source,
                })?;
                (blake3_hex(&bytes), Some(bytes))
            };
            files_by_hash
                .entry(file_hash.clone())
                .or_default()
                .push((rel_path.clone(), size_bytes));
            let changed = previous_row
                .map(|old| !metadata_unchanged && old.blake3 != file_hash)
                .unwrap_or(true);
            if changed {
                changed_files.push(rel_path.clone());
            }
            persist_file(
                &conn,
                rel_path,
                &file_hash,
                size_bytes,
                mtime_ns,
                scan_started_unix_ms,
            )?;
            if !changed {
                continue;
            }
            let bytes = bytes.ok_or_else(|| ProjectCacheError::Io {
                path: absolute.display().to_string(),
                source: std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "changed file missing content bytes",
                ),
            })?;
            delete_chunk_paths_for_file(&conn, rel_path)?;
            delete_imports_for_file(&conn, rel_path)?;
            for imported in python_imports(rel_path, &bytes) {
                conn.execute(
                    "INSERT OR IGNORE INTO imports(path, imported_module) VALUES (?1, ?2)",
                    params![rel_path, imported],
                )
                .map_err(|source| ProjectCacheError::Sqlite {
                    context: "insert import",
                    source,
                })?;
            }
            for chunk in chunks_for_file(rel_path, &bytes, &self.config.chunk_schema_version)? {
                chunks_to_refresh.insert(chunk.chunk_key.clone());
                persist_chunk(&conn, &chunk)?;
                conn.execute(
                    "INSERT OR REPLACE INTO chunk_paths(path, chunk_key, file_blake3) VALUES (?1, ?2, ?3)",
                    params![rel_path, chunk.chunk_key, file_hash],
                )
                .map_err(|source| ProjectCacheError::Sqlite {
                    context: "insert chunk path",
                    source,
                })?;
            }
        }

        reject_hash_collisions(&files_by_hash)?;
        for deleted in &deleted_files {
            conn.execute("DELETE FROM files WHERE path = ?1", params![deleted])
                .map_err(|source| ProjectCacheError::Sqlite {
                    context: "delete file row",
                    source,
                })?;
            delete_chunk_paths_for_file(&conn, deleted)?;
            delete_imports_for_file(&conn, deleted)?;
        }
        prune_orphan_chunks(&conn)?;
        queue_lazy_rescans(&conn, &changed_files, &deleted_files)?;
        let per_embedder_reembedded =
            refresh_embeddings(&conn, embedder_versions, &chunks_to_refresh)?;
        let merkle_root = merkle_root(&tracked, &files_by_hash);
        let status = WatcherStatus {
            project_id: self.config.project_id.clone(),
            heartbeat_unix_ms: scan_started_unix_ms,
            lag_count: 0,
            file_count: tracked.len(),
            merkle_root: merkle_root.clone(),
        };
        write_status(&self.status_path, &status)?;
        let report = ProjectCacheReport {
            project_id: self.config.project_id.clone(),
            repo_path: self.config.repo_path.display().to_string(),
            db_path: self.db_path.display().to_string(),
            watcher_status_path: self.status_path.display().to_string(),
            merkle_root,
            scan_started_unix_ms,
            file_count: count_rows(&conn, "files")?,
            chunk_path_count: count_rows(&conn, "chunk_paths")?,
            unique_chunk_count: count_rows(&conn, "chunks")?,
            embedding_count: count_rows(&conn, "embeddings")?,
            changed_files,
            deleted_files,
            deduped_chunk_paths: count_deduped_chunk_paths(&conn)?,
            lazy_rescan_queued: count_rows(&conn, "lazy_rescan_queue")?,
            per_embedder_reembedded,
        };
        Ok(report)
    }

    pub fn scan_after_debounce(
        &self,
        embedder_versions: &[EmbedderVersion],
    ) -> Result<ProjectCacheReport, ProjectCacheError> {
        std::thread::sleep(Duration::from_millis(PROJECT_CACHE_DEBOUNCE_MS));
        self.scan_project(embedder_versions)
    }

    pub fn scan_project_paths(
        &self,
        embedder_versions: &[EmbedderVersion],
        changed_paths: &[String],
    ) -> Result<ProjectCacheReport, ProjectCacheError> {
        let scan_started_unix_ms = now_ms();
        let conn = self.open_connection()?;
        let mut current = load_existing_files(&conn)?;
        let mut changed_files = Vec::new();
        let mut deleted_files = Vec::new();
        let mut chunks_to_refresh = BTreeSet::new();
        let mut requested = changed_paths
            .iter()
            .map(|path| normalize_requested_project_path(path))
            .collect::<Result<Vec<_>, _>>()?;
        requested.sort();
        requested.dedup();

        for rel_path in requested {
            if self
                .config
                .single_file
                .as_ref()
                .map(|only| only != &rel_path)
                .unwrap_or(false)
            {
                continue;
            }
            if !is_supported_project_file(&rel_path) || !self.config.scope.includes_path(&rel_path)
            {
                return Err(ProjectCacheError::UnsupportedFile { path: rel_path });
            }
            let absolute = self.config.repo_path.join(&rel_path);
            if !absolute.exists() {
                if current.remove(&rel_path).is_some() {
                    deleted_files.push(rel_path.clone());
                    conn.execute("DELETE FROM files WHERE path = ?1", params![rel_path])
                        .map_err(|source| ProjectCacheError::Sqlite {
                            context: "delete changed-path file row",
                            source,
                        })?;
                    delete_chunk_paths_for_file(&conn, &rel_path)?;
                    delete_imports_for_file(&conn, &rel_path)?;
                }
                continue;
            }
            let metadata = fs::metadata(&absolute).map_err(|source| ProjectCacheError::Io {
                path: absolute.display().to_string(),
                source,
            })?;
            if !metadata.is_file() {
                return Err(ProjectCacheError::UnsupportedFile { path: rel_path });
            }
            let bytes = fs::read(&absolute).map_err(|source| ProjectCacheError::Io {
                path: absolute.display().to_string(),
                source,
            })?;
            let file_hash = blake3_hex(&bytes);
            let size_bytes = metadata.len();
            let mtime_ns = metadata_modified_ns(&metadata);
            let changed = current
                .get(&rel_path)
                .map(|old| old.blake3 != file_hash)
                .unwrap_or(true);
            persist_file(
                &conn,
                &rel_path,
                &file_hash,
                size_bytes,
                mtime_ns,
                scan_started_unix_ms,
            )?;
            current.insert(
                rel_path.clone(),
                ExistingFile {
                    blake3: file_hash.clone(),
                    size_bytes,
                    mtime_ns,
                },
            );
            if !changed {
                continue;
            }
            changed_files.push(rel_path.clone());
            delete_chunk_paths_for_file(&conn, &rel_path)?;
            delete_imports_for_file(&conn, &rel_path)?;
            for imported in python_imports(&rel_path, &bytes) {
                conn.execute(
                    "INSERT OR IGNORE INTO imports(path, imported_module) VALUES (?1, ?2)",
                    params![rel_path, imported],
                )
                .map_err(|source| ProjectCacheError::Sqlite {
                    context: "insert changed-path import",
                    source,
                })?;
            }
            for chunk in chunks_for_file(&rel_path, &bytes, &self.config.chunk_schema_version)? {
                chunks_to_refresh.insert(chunk.chunk_key.clone());
                persist_chunk(&conn, &chunk)?;
                conn.execute(
                    "INSERT OR REPLACE INTO chunk_paths(path, chunk_key, file_blake3) VALUES (?1, ?2, ?3)",
                    params![rel_path, chunk.chunk_key, file_hash],
                )
                .map_err(|source| ProjectCacheError::Sqlite {
                    context: "insert changed-path chunk path",
                    source,
                })?;
            }
        }

        let files_by_hash = files_by_hash_from_existing(&current);
        reject_hash_collisions(&files_by_hash)?;
        prune_orphan_chunks(&conn)?;
        queue_lazy_rescans(&conn, &changed_files, &deleted_files)?;
        let per_embedder_reembedded =
            refresh_embeddings(&conn, embedder_versions, &chunks_to_refresh)?;
        let merkle_root = merkle_root_from_existing(&current);
        let status = WatcherStatus {
            project_id: self.config.project_id.clone(),
            heartbeat_unix_ms: scan_started_unix_ms,
            lag_count: 0,
            file_count: current.len(),
            merkle_root: merkle_root.clone(),
        };
        write_status(&self.status_path, &status)?;
        Ok(ProjectCacheReport {
            project_id: self.config.project_id.clone(),
            repo_path: self.config.repo_path.display().to_string(),
            db_path: self.db_path.display().to_string(),
            watcher_status_path: self.status_path.display().to_string(),
            merkle_root,
            scan_started_unix_ms,
            file_count: current.len(),
            chunk_path_count: 0,
            unique_chunk_count: 0,
            embedding_count: 0,
            changed_files,
            deleted_files,
            deduped_chunk_paths: 0,
            lazy_rescan_queued: 0,
            per_embedder_reembedded,
        })
    }

    fn init_schema(&self) -> Result<(), ProjectCacheError> {
        let conn = self.open_connection()?;
        conn.execute_batch(
            "\
            PRAGMA journal_mode = WAL;
            PRAGMA foreign_keys = ON;
            CREATE TABLE IF NOT EXISTS metadata(
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS files(
                path TEXT PRIMARY KEY,
                blake3 TEXT NOT NULL,
                size_bytes INTEGER NOT NULL,
                mtime_ns INTEGER NOT NULL DEFAULT 0,
                last_seen INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS chunks(
                chunk_key TEXT PRIMARY KEY,
                file_blake3 TEXT NOT NULL,
                role TEXT NOT NULL,
                kind TEXT NOT NULL,
                byte_start INTEGER NOT NULL,
                byte_end INTEGER NOT NULL,
                line_start INTEGER NOT NULL,
                line_end INTEGER NOT NULL,
                content_sha256 TEXT NOT NULL,
                updated_at INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS chunk_paths(
                path TEXT NOT NULL,
                chunk_key TEXT NOT NULL,
                file_blake3 TEXT NOT NULL,
                PRIMARY KEY(path, chunk_key),
                FOREIGN KEY(chunk_key) REFERENCES chunks(chunk_key) ON DELETE CASCADE
            );
            CREATE TABLE IF NOT EXISTS embeddings(
                chunk_key TEXT NOT NULL,
                embedder_id TEXT NOT NULL,
                embedder_version TEXT NOT NULL,
                vector_blob BLOB NOT NULL,
                updated_at INTEGER NOT NULL,
                PRIMARY KEY(chunk_key, embedder_id),
                FOREIGN KEY(chunk_key) REFERENCES chunks(chunk_key) ON DELETE CASCADE
            );
            CREATE TABLE IF NOT EXISTS imports(
                path TEXT NOT NULL,
                imported_module TEXT NOT NULL,
                PRIMARY KEY(path, imported_module)
            );
            CREATE TABLE IF NOT EXISTS lazy_rescan_queue(
                path TEXT PRIMARY KEY,
                reason TEXT NOT NULL,
                dependency_path TEXT NOT NULL,
                queued_at INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_files_blake3 ON files(blake3);
            CREATE INDEX IF NOT EXISTS idx_chunk_paths_chunk_key ON chunk_paths(chunk_key);
            CREATE INDEX IF NOT EXISTS idx_imports_module ON imports(imported_module);
            ",
        )
        .map_err(|source| ProjectCacheError::Sqlite {
            context: "initialize schema",
            source,
        })?;
        let has_mtime: Option<i64> = conn
            .query_row(
                "SELECT 1 FROM pragma_table_info('files') WHERE name = 'mtime_ns'",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(|source| ProjectCacheError::Sqlite {
                context: "inspect files mtime column",
                source,
            })?;
        if has_mtime.is_none() {
            conn.execute(
                "ALTER TABLE files ADD COLUMN mtime_ns INTEGER NOT NULL DEFAULT 0",
                [],
            )
            .map_err(|source| ProjectCacheError::Sqlite {
                context: "add files mtime column",
                source,
            })?;
        }
        conn.execute(
            "INSERT OR REPLACE INTO metadata(key, value) VALUES ('schema_version', ?1)",
            params![CACHE_SCHEMA_VERSION.to_string()],
        )
        .map_err(|source| ProjectCacheError::Sqlite {
            context: "write schema version",
            source,
        })?;
        Ok(())
    }

    fn open_connection(&self) -> Result<Connection, ProjectCacheError> {
        Connection::open(&self.db_path).map_err(|source| ProjectCacheError::Sqlite {
            context: "open cache db",
            source,
        })
    }
}

#[derive(Debug, Clone)]
struct ExistingFile {
    blake3: String,
    size_bytes: u64,
    mtime_ns: i64,
}

#[derive(Debug, Clone)]
struct CacheChunk {
    chunk_key: String,
    file_blake3: String,
    role: ChunkRole,
    kind: String,
    byte_start: u64,
    byte_end: u64,
    line_start: u32,
    line_end: u32,
    content_sha256: String,
}

fn canonicalize_existing_dir(path: &Path) -> Result<PathBuf, std::io::Error> {
    let canonical = fs::canonicalize(path)?;
    if canonical.is_dir() {
        Ok(canonical)
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "path is not a directory",
        ))
    }
}

fn require_project_cache_root(path: &Path) -> Result<PathBuf, ProjectCacheError> {
    let normalized = context_graph_paths::require_production_durable_root_or_named_local_smoke(
        path,
        "project_cache.cache_dir",
    )?;
    let production_projects =
        Path::new(context_graph_paths::AIWONDER_DURABLE_ROOT).join("projects");
    let legacy_projects = Path::new(context_graph_paths::LEGACY_LOCAL_DATA_ROOT).join("projects");
    if normalized.starts_with(&production_projects)
        || (context_graph_paths::named_local_smoke_issue().is_some()
            && normalized.starts_with(&legacy_projects))
    {
        Ok(normalized)
    } else {
        Err(ProjectCacheError::InvalidCacheRoot {
            path: normalized.display().to_string(),
        })
    }
}

fn project_files(repo_path: &Path) -> Result<Vec<String>, ProjectCacheError> {
    if is_git_work_tree(repo_path)? {
        return git_tracked_files(repo_path);
    }
    recursive_supported_files(repo_path)
}

fn is_git_work_tree(path: &Path) -> Result<bool, ProjectCacheError> {
    let output = Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .current_dir(path)
        .output()
        .map_err(|source| ProjectCacheError::Io {
            path: path.display().to_string(),
            source,
        })?;
    Ok(output.status.success() && String::from_utf8_lossy(&output.stdout).trim().eq("true"))
}

fn git_tracked_files(repo_path: &Path) -> Result<Vec<String>, ProjectCacheError> {
    let output = Command::new("git")
        .args([
            "ls-files",
            "--cached",
            "--others",
            "--exclude-standard",
            "-z",
        ])
        .current_dir(repo_path)
        .output()
        .map_err(|source| ProjectCacheError::Io {
            path: repo_path.display().to_string(),
            source,
        })?;
    if !output.status.success() {
        return Err(ProjectCacheError::GitFailed {
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    }
    let mut files = Vec::new();
    for raw in output.stdout.split(|byte| *byte == 0) {
        if raw.is_empty() {
            continue;
        }
        let rel = String::from_utf8_lossy(raw).replace('\\', "/");
        if is_supported_project_file(&rel) {
            files.push(rel);
        }
    }
    files.sort();
    Ok(files)
}

fn recursive_supported_files(root: &Path) -> Result<Vec<String>, ProjectCacheError> {
    let mut files = Vec::new();
    recursive_supported_files_inner(root, root, &mut files)?;
    files.sort();
    Ok(files)
}

fn recursive_supported_files_inner(
    root: &Path,
    dir: &Path,
    files: &mut Vec<String>,
) -> Result<(), ProjectCacheError> {
    for entry in fs::read_dir(dir).map_err(|source| ProjectCacheError::Io {
        path: dir.display().to_string(),
        source,
    })? {
        let entry = entry.map_err(|source| ProjectCacheError::Io {
            path: dir.display().to_string(),
            source,
        })?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|source| ProjectCacheError::Io {
            path: path.display().to_string(),
            source,
        })?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if file_type.is_dir() {
            if ignored_project_dir(&name) {
                continue;
            }
            recursive_supported_files_inner(root, &path, files)?;
        } else if file_type.is_file() {
            let rel = path
                .strip_prefix(root)
                .map_err(|source| ProjectCacheError::Io {
                    path: path.display().to_string(),
                    source: std::io::Error::new(std::io::ErrorKind::InvalidData, source),
                })?
                .to_string_lossy()
                .replace('\\', "/");
            if is_supported_project_file(&rel) {
                files.push(rel);
            }
        }
    }
    Ok(())
}

fn ignored_project_dir(name: &str) -> bool {
    matches!(
        name,
        ".git"
            | "target"
            | "__pycache__"
            | ".mypy_cache"
            | ".pytest_cache"
            | ".tox"
            | ".venv"
            | "venv"
            | "node_modules"
            | "dist"
            | "build"
    )
}

fn is_supported_project_file(path: &str) -> bool {
    path.ends_with(".py")
        || path.ends_with(".md")
        || path.ends_with(".toml")
        || path.ends_with(".json")
        || path.ends_with(".yaml")
        || path.ends_with(".yml")
        || path.ends_with(".txt")
}

pub fn project_role_for_path(path: &str) -> ChunkRole {
    if path.ends_with(".md") {
        ChunkRole::Doc
    } else if path.ends_with(".toml")
        || path.ends_with(".json")
        || path.ends_with(".yaml")
        || path.ends_with(".yml")
        || path.ends_with(".txt")
    {
        ChunkRole::Config
    } else if path.starts_with("tests/")
        || path.contains("/tests/")
        || path.rsplit('/').next().is_some_and(|name| {
            name == "test.py" || name.starts_with("test_") || name.contains("_test")
        })
    {
        ChunkRole::Test
    } else {
        ChunkRole::Source
    }
}

fn role_for_path(path: &str) -> ChunkRole {
    project_role_for_path(path)
}

fn chunks_for_file(
    rel_path: &str,
    bytes: &[u8],
    schema_version: &str,
) -> Result<Vec<CacheChunk>, ProjectCacheError> {
    let file_blake3 = blake3_hex(bytes);
    let role = role_for_path(rel_path);
    if rel_path.ends_with(".py") {
        let options = AstChunkOptions {
            file_path: rel_path.to_string(),
            max_non_ws_chars: 500,
        };
        let normalized = decode_utf8_or_latin1(bytes);
        let chunks =
            ast::chunk_with_options(normalized.as_bytes(), ast::Language::Python, &options)
                .map_err(|err| ProjectCacheError::ChunkFailed {
                    path: rel_path.to_string(),
                    reason: format!("{}: {err}", err.code()),
                })?;
        return chunks
            .iter()
            .map(|chunk| cache_chunk_from_ast(rel_path, &file_blake3, schema_version, chunk))
            .collect();
    }
    if bytes.is_empty() {
        return Ok(Vec::new());
    }
    let text = decode_utf8_or_latin1(bytes);
    let content_sha256 = sha256_hex(text.as_bytes());
    Ok(vec![CacheChunk {
        chunk_key: chunk_key(
            schema_version,
            &file_blake3,
            0,
            bytes.len() as u64,
            "whole_file",
        ),
        file_blake3,
        role,
        kind: "whole_file".to_string(),
        byte_start: 0,
        byte_end: bytes.len() as u64,
        line_start: 1,
        line_end: text.lines().count().max(1) as u32,
        content_sha256,
    }])
}

fn cache_chunk_from_ast(
    rel_path: &str,
    file_blake3: &str,
    schema_version: &str,
    chunk: &AstChunk,
) -> Result<CacheChunk, ProjectCacheError> {
    let kind = format!("{:?}", chunk.entity_type).to_ascii_lowercase();
    Ok(CacheChunk {
        chunk_key: chunk_key(
            schema_version,
            file_blake3,
            chunk.start_byte as u64,
            chunk.end_byte as u64,
            &kind,
        ),
        file_blake3: file_blake3.to_string(),
        role: role_for_path(rel_path),
        kind,
        byte_start: chunk.start_byte as u64,
        byte_end: chunk.end_byte as u64,
        line_start: chunk.line_start,
        line_end: chunk.line_end,
        content_sha256: chunk.sha256.clone(),
    })
}

fn load_existing_files(
    conn: &Connection,
) -> Result<BTreeMap<String, ExistingFile>, ProjectCacheError> {
    let mut stmt = conn
        .prepare("SELECT path, blake3, size_bytes, mtime_ns FROM files ORDER BY path")
        .map_err(|source| ProjectCacheError::Sqlite {
            context: "prepare existing files query",
            source,
        })?;
    let mut rows = stmt.query([]).map_err(|source| ProjectCacheError::Sqlite {
        context: "query existing files",
        source,
    })?;
    let mut out = BTreeMap::new();
    while let Some(row) = rows.next().map_err(|source| ProjectCacheError::Sqlite {
        context: "read existing file row",
        source,
    })? {
        let path: String = row.get(0).map_err(|source| ProjectCacheError::Sqlite {
            context: "decode existing file path",
            source,
        })?;
        let blake3: String = row.get(1).map_err(|source| ProjectCacheError::Sqlite {
            context: "decode existing file hash",
            source,
        })?;
        let size_bytes = row
            .get::<_, i64>(2)
            .map_err(|source| ProjectCacheError::Sqlite {
                context: "decode existing file size",
                source,
            })? as u64;
        let mtime_ns = row
            .get::<_, i64>(3)
            .map_err(|source| ProjectCacheError::Sqlite {
                context: "decode existing file mtime",
                source,
            })?;
        out.insert(
            path,
            ExistingFile {
                blake3,
                size_bytes,
                mtime_ns,
            },
        );
    }
    Ok(out)
}

fn persist_file(
    conn: &Connection,
    path: &str,
    file_hash: &str,
    size_bytes: u64,
    mtime_ns: i64,
    seen_at: i64,
) -> Result<(), ProjectCacheError> {
    conn.execute(
        "\
        INSERT INTO files(path, blake3, size_bytes, mtime_ns, last_seen)
        VALUES (?1, ?2, ?3, ?4, ?5)
        ON CONFLICT(path) DO UPDATE SET
            blake3 = excluded.blake3,
            size_bytes = excluded.size_bytes,
            mtime_ns = excluded.mtime_ns,
            last_seen = CASE WHEN files.blake3 = excluded.blake3 THEN files.last_seen ELSE excluded.last_seen END
        ",
        params![path, file_hash, size_bytes as i64, mtime_ns, seen_at],
    )
    .map_err(|source| ProjectCacheError::Sqlite {
        context: "upsert file row",
        source,
    })?;
    Ok(())
}

fn persist_chunk(conn: &Connection, chunk: &CacheChunk) -> Result<(), ProjectCacheError> {
    conn.execute(
        "\
        INSERT OR IGNORE INTO chunks(
            chunk_key, file_blake3, role, kind, byte_start, byte_end,
            line_start, line_end, content_sha256, updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            chunk.chunk_key,
            chunk.file_blake3,
            chunk.role.as_str(),
            chunk.kind,
            chunk.byte_start as i64,
            chunk.byte_end as i64,
            chunk.line_start as i64,
            chunk.line_end as i64,
            chunk.content_sha256,
            now_ms(),
        ],
    )
    .map_err(|source| ProjectCacheError::Sqlite {
        context: "insert chunk row",
        source,
    })?;
    Ok(())
}

fn delete_chunk_paths_for_file(conn: &Connection, path: &str) -> Result<(), ProjectCacheError> {
    conn.execute("DELETE FROM chunk_paths WHERE path = ?1", params![path])
        .map_err(|source| ProjectCacheError::Sqlite {
            context: "delete chunk paths for file",
            source,
        })?;
    Ok(())
}

fn delete_imports_for_file(conn: &Connection, path: &str) -> Result<(), ProjectCacheError> {
    conn.execute("DELETE FROM imports WHERE path = ?1", params![path])
        .map_err(|source| ProjectCacheError::Sqlite {
            context: "delete imports for file",
            source,
        })?;
    Ok(())
}

fn prune_orphan_chunks(conn: &Connection) -> Result<(), ProjectCacheError> {
    conn.execute(
        "DELETE FROM embeddings WHERE chunk_key NOT IN (SELECT chunk_key FROM chunk_paths)",
        [],
    )
    .map_err(|source| ProjectCacheError::Sqlite {
        context: "delete orphan embeddings",
        source,
    })?;
    conn.execute(
        "DELETE FROM chunks WHERE chunk_key NOT IN (SELECT chunk_key FROM chunk_paths)",
        [],
    )
    .map_err(|source| ProjectCacheError::Sqlite {
        context: "delete orphan chunks",
        source,
    })?;
    Ok(())
}

fn queue_lazy_rescans(
    conn: &Connection,
    changed_files: &[String],
    deleted_files: &[String],
) -> Result<(), ProjectCacheError> {
    let mut affected_modules = BTreeMap::new();
    for path in changed_files.iter().chain(deleted_files.iter()) {
        if let Some(module) = module_name_for_path(path) {
            affected_modules.insert(module, path.clone());
        }
    }
    for (module, dependency_path) in affected_modules {
        let mut stmt = conn
            .prepare("SELECT path FROM imports WHERE imported_module = ?1 ORDER BY path")
            .map_err(|source| ProjectCacheError::Sqlite {
                context: "prepare import dependent query",
                source,
            })?;
        let rows = stmt
            .query_map(params![module], |row| row.get::<_, String>(0))
            .map_err(|source| ProjectCacheError::Sqlite {
                context: "query import dependents",
                source,
            })?;
        for row in rows {
            let path = row.map_err(|source| ProjectCacheError::Sqlite {
                context: "read import dependent",
                source,
            })?;
            if path != dependency_path {
                conn.execute(
                    "\
                    INSERT OR REPLACE INTO lazy_rescan_queue(path, reason, dependency_path, queued_at)
                    VALUES (?1, 'lazy_transitive_dependency_changed', ?2, ?3)",
                    params![path, dependency_path, now_ms()],
                )
                .map_err(|source| ProjectCacheError::Sqlite {
                    context: "insert lazy rescan",
                    source,
                })?;
            }
        }
    }
    Ok(())
}

fn refresh_embeddings(
    conn: &Connection,
    embedder_versions: &[EmbedderVersion],
    chunk_keys: &BTreeSet<String>,
) -> Result<BTreeMap<String, usize>, ProjectCacheError> {
    let mut per_embedder = BTreeMap::new();
    for chunk_key in chunk_keys {
        for embedder in embedder_versions {
            let existing: Option<String> = conn
                .query_row(
                    "SELECT embedder_version FROM embeddings WHERE chunk_key = ?1 AND embedder_id = ?2",
                    params![chunk_key, embedder.embedder_id],
                    |row| row.get(0),
                )
                .optional()
                .map_err(|source| ProjectCacheError::Sqlite {
                    context: "query embedding version",
                    source,
                })?;
            if existing.as_deref() == Some(embedder.embedder_version.as_str()) {
                continue;
            }
            let vector = deterministic_vector_blob(
                chunk_key,
                &embedder.embedder_id,
                &embedder.embedder_version,
            );
            conn.execute(
                "\
                INSERT INTO embeddings(chunk_key, embedder_id, embedder_version, vector_blob, updated_at)
                VALUES (?1, ?2, ?3, ?4, ?5)
                ON CONFLICT(chunk_key, embedder_id) DO UPDATE SET
                    embedder_version = excluded.embedder_version,
                    vector_blob = excluded.vector_blob,
                    updated_at = excluded.updated_at",
                params![
                    chunk_key,
                    embedder.embedder_id,
                    embedder.embedder_version,
                    vector,
                    now_ms(),
                ],
            )
            .map_err(|source| ProjectCacheError::Sqlite {
                context: "upsert embedding row",
                source,
            })?;
            *per_embedder
                .entry(embedder.embedder_id.clone())
                .or_insert(0) += 1;
        }
    }
    Ok(per_embedder)
}

fn reject_hash_collisions(
    files_by_hash: &BTreeMap<String, Vec<(String, u64)>>,
) -> Result<(), ProjectCacheError> {
    for (hash, rows) in files_by_hash {
        let sizes: BTreeSet<u64> = rows.iter().map(|(_, size)| *size).collect();
        if sizes.len() > 1 {
            return Err(ProjectCacheError::HashCollision {
                blake3: hash.clone(),
                paths: rows.iter().map(|(path, _)| path.clone()).collect(),
            });
        }
    }
    Ok(())
}

fn python_imports(rel_path: &str, bytes: &[u8]) -> Vec<String> {
    if !rel_path.ends_with(".py") {
        return Vec::new();
    }
    let Ok(text) = std::str::from_utf8(bytes) else {
        return Vec::new();
    };
    let mut imports = BTreeSet::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("import ") {
            for part in rest.split(',') {
                if let Some(name) = part.split_whitespace().next() {
                    imports.insert(name.split('.').next().unwrap_or(name).to_string());
                }
            }
        } else if let Some(rest) = trimmed.strip_prefix("from ") {
            if let Some(name) = rest.split_whitespace().next() {
                if !name.starts_with('.') {
                    imports.insert(name.split('.').next().unwrap_or(name).to_string());
                }
            }
        }
    }
    imports.into_iter().collect()
}

fn module_name_for_path(path: &str) -> Option<String> {
    path.rsplit('/')
        .next()
        .and_then(|name| name.strip_suffix(".py"))
        .filter(|name| !name.is_empty() && *name != "__init__")
        .map(ToString::to_string)
}

fn write_status(path: &Path, status: &WatcherStatus) -> Result<(), ProjectCacheError> {
    let bytes = serde_json::to_vec_pretty(status).map_err(|source| ProjectCacheError::Io {
        path: path.display().to_string(),
        source: std::io::Error::new(std::io::ErrorKind::InvalidData, source),
    })?;
    fs::write(path, bytes).map_err(|source| ProjectCacheError::Io {
        path: path.display().to_string(),
        source,
    })?;
    Ok(())
}

fn count_rows(conn: &Connection, table: &str) -> Result<usize, ProjectCacheError> {
    let sql = format!("SELECT COUNT(*) FROM {table}");
    let count: i64 = conn
        .query_row(&sql, [], |row| row.get(0))
        .map_err(|source| ProjectCacheError::Sqlite {
            context: "count rows",
            source,
        })?;
    Ok(count as usize)
}

fn count_deduped_chunk_paths(conn: &Connection) -> Result<usize, ProjectCacheError> {
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM (SELECT chunk_key FROM chunk_paths GROUP BY chunk_key HAVING COUNT(*) > 1)",
            [],
            |row| row.get(0),
        )
        .map_err(|source| ProjectCacheError::Sqlite {
            context: "count deduped chunk paths",
            source,
        })?;
    Ok(count as usize)
}

fn merkle_root(tracked: &[String], files_by_hash: &BTreeMap<String, Vec<(String, u64)>>) -> String {
    let mut leaves = Vec::new();
    for path in tracked {
        let hash = files_by_hash
            .iter()
            .find_map(|(hash, rows)| {
                rows.iter()
                    .any(|(row_path, _)| row_path == path)
                    .then_some(hash)
            })
            .cloned()
            .unwrap_or_default();
        leaves.push(format!("{path}\0{hash}"));
    }
    blake3_hex(leaves.join("\n").as_bytes())
}

fn merkle_root_from_existing(files: &BTreeMap<String, ExistingFile>) -> String {
    let leaves = files
        .iter()
        .map(|(path, file)| format!("{path}\0{}", file.blake3))
        .collect::<Vec<_>>();
    blake3_hex(leaves.join("\n").as_bytes())
}

fn files_by_hash_from_existing(
    files: &BTreeMap<String, ExistingFile>,
) -> BTreeMap<String, Vec<(String, u64)>> {
    let mut out = BTreeMap::new();
    for (path, file) in files {
        out.entry(file.blake3.clone())
            .or_insert_with(Vec::new)
            .push((path.clone(), file.size_bytes));
    }
    out
}

fn normalize_requested_project_path(path: &str) -> Result<String, ProjectCacheError> {
    let rel = path.replace('\\', "/");
    if rel.is_empty()
        || rel.starts_with('/')
        || rel
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == ".." || part.contains('\0'))
    {
        return Err(ProjectCacheError::UnsupportedFile { path: rel });
    }
    Ok(rel)
}

fn chunk_key(schema_version: &str, file_blake3: &str, start: u64, end: u64, kind: &str) -> String {
    blake3_hex(format!("{schema_version}\0{file_blake3}\0{start}\0{end}\0{kind}").as_bytes())
}

fn deterministic_vector_blob(chunk_key: &str, embedder_id: &str, version: &str) -> Vec<u8> {
    blake3::hash(format!("{chunk_key}\0{embedder_id}\0{version}").as_bytes())
        .as_bytes()
        .to_vec()
}

fn decode_utf8_or_latin1(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(text) => text.to_string(),
        Err(_) => bytes.iter().map(|byte| *byte as char).collect(),
    }
}

pub fn blake3_hex(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes))
}

fn metadata_modified_ns(metadata: &fs::Metadata) -> i64 {
    metadata
        .modified()
        .ok()
        .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos().min(i64::MAX as u128) as i64)
        .unwrap_or(0)
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}
