use crate::calibration::open_infer_rocksdb;
use crate::cli::write_json_0600;
use crate::compiler::MejepaStore;
use crate::error::MejepaInferError;
use crate::project_cache::{
    blake3_hex, project_role_for_path, ChunkRole, EmbedderVersion, ProjectCacheConfig,
    ProjectCacheError, ProjectFileScope, ProjectMerkleCache,
};
use crate::store::RocksDbInferStore;
use crate::types::{
    ChunkId, ConformalSet, EdgeCaseClass, EmbedderId, FailureModeClass, Language, LatentBugClass,
    OracleOutcome, PredictedEdgeCase, PredictedFailureMode, PredictedLatentBug,
    PredictedSecurityConcern, PredictedWorks, PredictionProvenance, RealityPrediction,
    RealityPredictionBuilder, RootCauseClass, SecurityConcernClass, Severity, TaskId, Verdict,
    WitnessHash,
};
use context_graph_mejepa_cf::CF_MEJEPA_LIVE_PREDICTIONS;
use rocksdb::{IteratorMode, DB};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

const PROJECT_ROOT: &str = "/mnt/d/contextgraph/projects";
const PREDICTOR_VERSION: &str = "task-py-g-063-project-ingest-v1";

#[derive(Debug, Clone)]
struct ProjectSourcePath {
    input_path: PathBuf,
    source_root: PathBuf,
    single_file: Option<String>,
}

#[derive(Debug, Error)]
pub enum ProjectIngestError {
    #[error("MEJEPA_PROJECT_INGEST_INVALID_REPO_PATH: {path}")]
    InvalidRepoPath { path: String },
    #[error("MEJEPA_PROJECT_INGEST_REPO_PATH_DENIED: {path}")]
    RepoPathDenied { path: String },
    #[error("MEJEPA_PROJECT_INGEST_INVALID_PROJECT_ID: {project_id}")]
    InvalidProjectId { project_id: String },
    #[error("MEJEPA_PROJECT_INGEST_PROJECT_COLLISION: project_id={project_id} existing_repo={existing_repo_path}")]
    ProjectCollision {
        project_id: String,
        existing_repo_path: String,
    },
    #[error("MEJEPA_PROJECT_INGEST_INCREMENTAL_WITHOUT_MANIFEST: project_id={project_id}")]
    IncrementalWithoutManifest { project_id: String },
    #[error("MEJEPA_PROJECT_INGEST_BINARY_REJECTED: {path}")]
    BinaryRejected { path: String },
    #[error("MEJEPA_PROJECT_INGEST_MISSING_PREDICTION_ROW: project_id={project_id} file_path={file_path} key={key_hex}")]
    MissingPredictionRow {
        project_id: String,
        file_path: String,
        key_hex: String,
    },
    #[error("MEJEPA_PROJECT_INGEST_GIT_FAILED: {stderr}")]
    GitFailed { stderr: String },
    #[error("MEJEPA_PROJECT_INGEST_IO: {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("MEJEPA_PROJECT_INGEST_SQLITE: {context}: {source}")]
    Sqlite {
        context: &'static str,
        #[source]
        source: rusqlite::Error,
    },
    #[error("MEJEPA_PROJECT_INGEST_JSON: {path}: {source}")]
    Json {
        path: String,
        #[source]
        source: serde_json::Error,
    },
    #[error("{0}")]
    Cache(#[from] ProjectCacheError),
    #[error("{0}")]
    Infer(#[from] MejepaInferError),
    #[error("MEJEPA_PROJECT_INGEST_ROCKSDB: {0}")]
    RocksDb(#[from] rocksdb::Error),
    #[error("MEJEPA_PROJECT_INGEST_BINCODE: {0}")]
    Bincode(#[from] Box<bincode::ErrorKind>),
}

impl ProjectIngestError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidRepoPath { .. } => "MEJEPA_PROJECT_INGEST_INVALID_REPO_PATH",
            Self::RepoPathDenied { .. } => "MEJEPA_PROJECT_INGEST_REPO_PATH_DENIED",
            Self::InvalidProjectId { .. } => "MEJEPA_PROJECT_INGEST_INVALID_PROJECT_ID",
            Self::ProjectCollision { .. } => "MEJEPA_PROJECT_INGEST_PROJECT_COLLISION",
            Self::IncrementalWithoutManifest { .. } => {
                "MEJEPA_PROJECT_INGEST_INCREMENTAL_WITHOUT_MANIFEST"
            }
            Self::BinaryRejected { .. } => "MEJEPA_PROJECT_INGEST_BINARY_REJECTED",
            Self::MissingPredictionRow { .. } => "MEJEPA_PROJECT_INGEST_MISSING_PREDICTION_ROW",
            Self::GitFailed { .. } => "MEJEPA_PROJECT_INGEST_GIT_FAILED",
            Self::Io { .. } => "MEJEPA_PROJECT_INGEST_IO",
            Self::Sqlite { .. } => "MEJEPA_PROJECT_INGEST_SQLITE",
            Self::Json { .. } => "MEJEPA_PROJECT_INGEST_JSON",
            Self::Cache(err) => err.code(),
            Self::Infer(err) => err.code(),
            Self::RocksDb(_) => "MEJEPA_PROJECT_INGEST_ROCKSDB",
            Self::Bincode(_) => "MEJEPA_PROJECT_INGEST_BINCODE",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum ProjectIngestMode {
    #[default]
    Full,
    Incremental,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum ProjectIngestScope {
    #[default]
    SourceOnly,
    SourceAndTests,
    All,
}

impl From<ProjectIngestScope> for ProjectFileScope {
    fn from(value: ProjectIngestScope) -> Self {
        match value {
            ProjectIngestScope::SourceOnly => Self::SourceOnly,
            ProjectIngestScope::SourceAndTests => Self::SourceAndTests,
            ProjectIngestScope::All => Self::All,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectIngestRequest {
    pub repo_path: PathBuf,
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub mode: ProjectIngestMode,
    #[serde(default)]
    pub scope: ProjectIngestScope,
    #[serde(default)]
    pub overwrite: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectIngestReport {
    pub schema_version: u32,
    pub project_id: String,
    pub project_root: String,
    pub manifest_path: String,
    pub cache_db_path: String,
    pub predictions_db_path: String,
    pub live_prediction_cf: String,
    pub mode: ProjectIngestMode,
    pub scope: ProjectIngestScope,
    pub cache_report: crate::project_cache::ProjectCacheReport,
    pub files_before: usize,
    pub files_after: usize,
    pub project_prediction_rows_before: usize,
    pub project_prediction_rows_after: usize,
    pub predictions_written: usize,
    pub new_embeddings_written: usize,
    pub manifest: ProjectIngestManifest,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectIngestManifest {
    pub schema_version: u32,
    pub project_id: String,
    pub repo_path: String,
    pub project_root: String,
    pub mode: ProjectIngestMode,
    pub scope: ProjectIngestScope,
    pub last_ingest_unix_ms: i64,
    pub merkle_root: String,
    pub file_count: usize,
    pub source_file_count: usize,
    pub test_file_count: usize,
    pub doc_file_count: usize,
    pub config_file_count: usize,
    pub prediction_count: usize,
    pub cache_db_path: String,
    pub predictions_db_path: String,
    pub predictions: Vec<ProjectPredictionManifestRow>,
    pub changed_files: Vec<String>,
    pub deleted_files: Vec<String>,
    pub new_embeddings_written: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectPredictionManifestRow {
    pub file_path: String,
    pub file_blake3: String,
    pub role: ChunkRole,
    pub size_bytes: u64,
    pub chunk_count: usize,
    pub prediction_id_hex: String,
    pub verdict: Verdict,
    pub predicted_failure_modes: usize,
    pub predicted_security_concerns: usize,
    pub task_id: String,
    #[serde(default)]
    pub live_prediction_key_hex: String,
}

#[derive(Debug, Clone)]
struct CachedFileRow {
    path: String,
    blake3: String,
    size_bytes: u64,
    role: ChunkRole,
    chunk_count: usize,
}

pub fn run_project_ingest(
    request: ProjectIngestRequest,
) -> Result<ProjectIngestReport, ProjectIngestError> {
    let source = canonicalize_source_path(&request.repo_path)?;
    require_operator_permitted_root(&source.input_path)?;
    validate_scope_content(
        &source.source_root,
        source.single_file.as_deref(),
        request.scope,
    )?;

    let project_id = match &request.project_id {
        Some(value) => validate_project_id(value)?,
        None => derive_project_id(&source.input_path),
    };
    let project_root = PathBuf::from(PROJECT_ROOT).join(&project_id);
    let manifest_path = project_root.join("manifest.json");
    let previous_manifest = read_manifest_if_present(&manifest_path)?;
    validate_collision_policy(
        request.mode,
        request.overwrite,
        &project_id,
        &source.input_path,
        previous_manifest.as_ref(),
    )?;

    let cache_dir = project_root.join("cache");
    let panels_dir = project_root.join("panels");
    let predictions_dir = project_root.join("predictions");
    let report_dir = project_root.join("report");
    for dir in [&cache_dir, &panels_dir, &predictions_dir, &report_dir] {
        fs::create_dir_all(dir).map_err(|source| ProjectIngestError::Io {
            path: dir.display().to_string(),
            source,
        })?;
    }
    let predictions_db_path = predictions_dir.join("live-predictions.rocksdb");
    if let Some(parent) = predictions_db_path.parent() {
        fs::create_dir_all(parent).map_err(|source| ProjectIngestError::Io {
            path: parent.display().to_string(),
            source,
        })?;
    }

    let cache = ProjectMerkleCache::open(
        ProjectCacheConfig::new(project_id.clone(), source.source_root.clone())
            .with_scope(request.scope.into())
            .with_single_file(source.single_file.clone()),
    )?;
    let files_before = cached_file_count(cache.db_path()).unwrap_or(0);
    let cache_report = cache.scan_project(&default_embedder_versions())?;
    let files_after = cached_file_count(cache.db_path())?;
    let cached_files = load_cached_files(cache.db_path())?;
    let db = open_infer_rocksdb(&predictions_db_path)?;
    let project_prediction_rows_before = count_project_predictions(db.as_ref(), &project_id)?;
    let store = RocksDbInferStore::new(db.clone());
    let changed: BTreeSet<_> = cache_report.changed_files.iter().cloned().collect();
    let deleted: BTreeSet<_> = cache_report.deleted_files.iter().cloned().collect();
    let mut prediction_rows = if request.mode == ProjectIngestMode::Incremental {
        previous_manifest
            .as_ref()
            .map(|manifest| {
                verify_retained_prediction_rows(
                    db.as_ref(),
                    &project_id,
                    manifest,
                    &changed,
                    &deleted,
                )
            })
            .transpose()?
            .unwrap_or_default()
    } else {
        BTreeMap::new()
    };
    let mut predictions_written = 0usize;
    for file in &cached_files {
        let should_predict = match request.mode {
            ProjectIngestMode::Full => true,
            ProjectIngestMode::Incremental => changed.contains(&file.path),
        };
        if !should_predict {
            continue;
        }
        let prediction = prediction_for_file(
            &project_id,
            &source.source_root,
            file,
            &cache_report.merkle_root,
        )?;
        let row = ProjectPredictionManifestRow {
            file_path: file.path.clone(),
            file_blake3: file.blake3.clone(),
            role: file.role.clone(),
            size_bytes: file.size_bytes,
            chunk_count: file.chunk_count,
            prediction_id_hex: hex::encode(prediction.prediction_id),
            verdict: prediction.verdict,
            predicted_failure_modes: prediction.predicted_failure_modes.len(),
            predicted_security_concerns: prediction.predicted_security_concerns.len(),
            task_id: prediction.task_id.0.clone(),
            live_prediction_key_hex: hex::encode(project_prediction_live_key(
                &project_id,
                &file.path,
                &file.blake3,
            )),
        };
        store.write_live_prediction(&prediction)?;
        prediction_rows.insert(file.path.clone(), row);
        predictions_written += 1;
    }
    let project_prediction_rows_after = count_project_predictions(db.as_ref(), &project_id)?;
    let mut predictions = prediction_rows.into_values().collect::<Vec<_>>();
    predictions.sort_by(|left, right| left.file_path.cmp(&right.file_path));
    let counts = role_counts(&cached_files);
    let new_embeddings_written = cache_report.per_embedder_reembedded.values().sum();
    let manifest = ProjectIngestManifest {
        schema_version: 1,
        project_id: project_id.clone(),
        repo_path: source.input_path.display().to_string(),
        project_root: project_root.display().to_string(),
        mode: request.mode,
        scope: request.scope,
        last_ingest_unix_ms: now_ms(),
        merkle_root: cache_report.merkle_root.clone(),
        file_count: cached_files.len(),
        source_file_count: counts.source,
        test_file_count: counts.test,
        doc_file_count: counts.doc,
        config_file_count: counts.config,
        prediction_count: predictions.len(),
        cache_db_path: cache.db_path().display().to_string(),
        predictions_db_path: predictions_db_path.display().to_string(),
        predictions,
        changed_files: cache_report.changed_files.clone(),
        deleted_files: cache_report.deleted_files.clone(),
        new_embeddings_written,
    };
    write_json_0600(&manifest_path, &manifest)?;
    let readback = read_manifest(&manifest_path)?;
    if readback.project_id != manifest.project_id
        || readback.last_ingest_unix_ms != manifest.last_ingest_unix_ms
        || readback.prediction_count != manifest.prediction_count
    {
        return Err(ProjectIngestError::Infer(MejepaInferError::InvalidInput {
            field: "project_manifest".to_string(),
            detail: "manifest readback did not match written manifest summary".to_string(),
        }));
    }
    Ok(ProjectIngestReport {
        schema_version: 1,
        project_id,
        project_root: project_root.display().to_string(),
        manifest_path: manifest_path.display().to_string(),
        cache_db_path: cache.db_path().display().to_string(),
        predictions_db_path: predictions_db_path.display().to_string(),
        live_prediction_cf: CF_MEJEPA_LIVE_PREDICTIONS.to_string(),
        mode: request.mode,
        scope: request.scope,
        cache_report,
        files_before,
        files_after,
        project_prediction_rows_before,
        project_prediction_rows_after,
        predictions_written,
        new_embeddings_written,
        manifest,
    })
}

pub fn count_project_predictions(db: &DB, project_id: &str) -> Result<usize, ProjectIngestError> {
    let cf =
        db.cf_handle(CF_MEJEPA_LIVE_PREDICTIONS)
            .ok_or_else(|| MejepaInferError::InvalidInput {
                field: "rocksdb.column_family".to_string(),
                detail: format!("missing column family {CF_MEJEPA_LIVE_PREDICTIONS}"),
            })?;
    let prefix = format!("project_ingest:{project_id}:");
    let mut count = 0usize;
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let (_key, value) = item?;
        let prediction: RealityPrediction = bincode::deserialize(&value)?;
        if prediction.task_id.0.starts_with(&prefix)
            && prediction.provenance.predictor_version == PREDICTOR_VERSION
        {
            count += 1;
        }
    }
    Ok(count)
}

pub fn project_prediction_id(project_id: &str, file_path: &str, file_blake3: &str) -> [u8; 16] {
    id16(&format!(
        "prediction\0{project_id}\0{file_path}\0{file_blake3}"
    ))
}

pub fn project_prediction_session_id(project_id: &str) -> [u8; 16] {
    id16(&format!("session\0{project_id}"))
}

pub fn project_prediction_live_key(
    project_id: &str,
    file_path: &str,
    file_blake3: &str,
) -> Vec<u8> {
    let mut key = Vec::with_capacity(40);
    key.extend_from_slice(&project_prediction_session_id(project_id));
    key.extend_from_slice(
        &stable_prediction_timestamp(project_id, file_path, file_blake3).to_be_bytes(),
    );
    key.extend_from_slice(&project_prediction_id(project_id, file_path, file_blake3));
    key
}

fn verify_retained_prediction_rows(
    db: &DB,
    project_id: &str,
    previous_manifest: &ProjectIngestManifest,
    changed: &BTreeSet<String>,
    deleted: &BTreeSet<String>,
) -> Result<BTreeMap<String, ProjectPredictionManifestRow>, ProjectIngestError> {
    let cf =
        db.cf_handle(CF_MEJEPA_LIVE_PREDICTIONS)
            .ok_or_else(|| MejepaInferError::InvalidInput {
                field: "rocksdb.column_family".to_string(),
                detail: format!("missing column family {CF_MEJEPA_LIVE_PREDICTIONS}"),
            })?;
    let mut retained = BTreeMap::new();
    for row in &previous_manifest.predictions {
        if deleted.contains(&row.file_path) || changed.contains(&row.file_path) {
            continue;
        }
        let key = project_prediction_live_key(project_id, &row.file_path, &row.file_blake3);
        let key_hex = hex::encode(&key);
        if !row.live_prediction_key_hex.is_empty() && row.live_prediction_key_hex != key_hex {
            return Err(ProjectIngestError::Infer(MejepaInferError::InvalidInput {
                field: "project_manifest.predictions.live_prediction_key_hex".to_string(),
                detail: format!(
                    "manifest key for {} does not match deterministic project/file/hash key",
                    row.file_path
                ),
            }));
        }
        let value =
            db.get_cf(cf, &key)?
                .ok_or_else(|| ProjectIngestError::MissingPredictionRow {
                    project_id: project_id.to_string(),
                    file_path: row.file_path.clone(),
                    key_hex: key_hex.clone(),
                })?;
        let prediction: RealityPrediction = bincode::deserialize(&value)?;
        prediction.validate().map_err(ProjectIngestError::Infer)?;
        if hex::encode(prediction.prediction_id) != row.prediction_id_hex
            || prediction.task_id.0 != row.task_id
            || prediction.provenance.predictor_version != PREDICTOR_VERSION
        {
            return Err(ProjectIngestError::Infer(MejepaInferError::InvalidInput {
                field: "live_predictions.retained_row".to_string(),
                detail: format!(
                    "retained prediction row for {} did not match manifest task/id/provenance",
                    row.file_path
                ),
            }));
        }
        let mut row = row.clone();
        row.live_prediction_key_hex = key_hex;
        retained.insert(row.file_path.clone(), row);
    }
    Ok(retained)
}

fn read_manifest_if_present(
    path: &Path,
) -> Result<Option<ProjectIngestManifest>, ProjectIngestError> {
    if !path.exists() {
        return Ok(None);
    }
    read_manifest(path).map(Some)
}

fn read_manifest(path: &Path) -> Result<ProjectIngestManifest, ProjectIngestError> {
    let bytes = fs::read(path).map_err(|source| ProjectIngestError::Io {
        path: path.display().to_string(),
        source,
    })?;
    serde_json::from_slice(&bytes).map_err(|source| ProjectIngestError::Json {
        path: path.display().to_string(),
        source,
    })
}

fn validate_collision_policy(
    mode: ProjectIngestMode,
    overwrite: bool,
    project_id: &str,
    repo_path: &Path,
    previous_manifest: Option<&ProjectIngestManifest>,
) -> Result<(), ProjectIngestError> {
    let Some(previous) = previous_manifest else {
        if mode == ProjectIngestMode::Incremental {
            return Err(ProjectIngestError::IncrementalWithoutManifest {
                project_id: project_id.to_string(),
            });
        }
        return Ok(());
    };
    if mode == ProjectIngestMode::Full && !overwrite {
        return Err(ProjectIngestError::ProjectCollision {
            project_id: project_id.to_string(),
            existing_repo_path: previous.repo_path.clone(),
        });
    }
    if previous.repo_path != repo_path.display().to_string() && !overwrite {
        return Err(ProjectIngestError::ProjectCollision {
            project_id: project_id.to_string(),
            existing_repo_path: previous.repo_path.clone(),
        });
    }
    Ok(())
}

fn canonicalize_source_path(path: &Path) -> Result<ProjectSourcePath, ProjectIngestError> {
    let canonical = fs::canonicalize(path).map_err(|source| ProjectIngestError::Io {
        path: path.display().to_string(),
        source,
    })?;
    if canonical.is_dir() {
        return Ok(ProjectSourcePath {
            input_path: canonical.clone(),
            source_root: canonical,
            single_file: None,
        });
    }
    if canonical.is_file() {
        let source_root = canonical.parent().map(Path::to_path_buf).ok_or_else(|| {
            ProjectIngestError::InvalidRepoPath {
                path: path.display().to_string(),
            }
        })?;
        let file_name = canonical
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| ProjectIngestError::InvalidRepoPath {
                path: path.display().to_string(),
            })?
            .replace('\\', "/");
        return Ok(ProjectSourcePath {
            input_path: canonical,
            source_root,
            single_file: Some(file_name),
        });
    }
    Err(ProjectIngestError::InvalidRepoPath {
        path: path.display().to_string(),
    })
}

fn require_operator_permitted_root(path: &Path) -> Result<(), ProjectIngestError> {
    let allowed = [
        Path::new("/home/cabdru"),
        Path::new("/mnt/d/contextgraph"),
        Path::new("/zfs/hot/contextgraph"),
        Path::new("/zfs/archive/contextgraph"),
    ];
    if allowed.iter().any(|root| path.starts_with(root)) {
        Ok(())
    } else {
        Err(ProjectIngestError::RepoPathDenied {
            path: path.display().to_string(),
        })
    }
}

fn validate_scope_content(
    source_root: &Path,
    single_file: Option<&str>,
    scope: ProjectIngestScope,
) -> Result<(), ProjectIngestError> {
    for rel_path in project_supported_files(source_root, single_file, scope)? {
        let absolute = source_root.join(&rel_path);
        let bytes = fs::read(&absolute).map_err(|source| ProjectIngestError::Io {
            path: absolute.display().to_string(),
            source,
        })?;
        if is_binary(&bytes) {
            return Err(ProjectIngestError::BinaryRejected { path: rel_path });
        }
    }
    Ok(())
}

fn project_supported_files(
    source_root: &Path,
    single_file: Option<&str>,
    scope: ProjectIngestScope,
) -> Result<Vec<String>, ProjectIngestError> {
    if let Some(single_file) = single_file {
        let single_file = single_file.replace('\\', "/");
        let project_scope: ProjectFileScope = scope.into();
        return Ok((is_supported_project_file(&single_file)
            && project_scope.includes_path(&single_file))
        .then_some(single_file)
        .into_iter()
        .collect());
    }
    if is_git_work_tree(source_root)? {
        return git_tracked_supported_files(source_root, scope);
    }
    recursive_supported_files(source_root, scope)
}

fn is_git_work_tree(path: &Path) -> Result<bool, ProjectIngestError> {
    let output = Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .current_dir(path)
        .output()
        .map_err(|source| ProjectIngestError::Io {
            path: path.display().to_string(),
            source,
        })?;
    Ok(output.status.success() && String::from_utf8_lossy(&output.stdout).trim().eq("true"))
}

fn git_tracked_supported_files(
    source_root: &Path,
    scope: ProjectIngestScope,
) -> Result<Vec<String>, ProjectIngestError> {
    let output = Command::new("git")
        .args([
            "ls-files",
            "--cached",
            "--others",
            "--exclude-standard",
            "-z",
        ])
        .current_dir(source_root)
        .output()
        .map_err(|source| ProjectIngestError::Io {
            path: source_root.display().to_string(),
            source,
        })?;
    if !output.status.success() {
        return Err(ProjectIngestError::GitFailed {
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    }
    let project_scope: ProjectFileScope = scope.into();
    let mut files = Vec::new();
    for raw in output.stdout.split(|byte| *byte == 0) {
        if raw.is_empty() {
            continue;
        }
        let rel = String::from_utf8_lossy(raw).replace('\\', "/");
        if is_supported_project_file(&rel) && project_scope.includes_path(&rel) {
            files.push(rel);
        }
    }
    files.sort();
    Ok(files)
}

fn recursive_supported_files(
    source_root: &Path,
    scope: ProjectIngestScope,
) -> Result<Vec<String>, ProjectIngestError> {
    let project_scope: ProjectFileScope = scope.into();
    let mut files = Vec::new();
    recursive_supported_files_inner(source_root, source_root, project_scope, &mut files)?;
    files.sort();
    Ok(files)
}

fn recursive_supported_files_inner(
    source_root: &Path,
    dir: &Path,
    scope: ProjectFileScope,
    files: &mut Vec<String>,
) -> Result<(), ProjectIngestError> {
    for entry in fs::read_dir(dir).map_err(|source| ProjectIngestError::Io {
        path: dir.display().to_string(),
        source,
    })? {
        let entry = entry.map_err(|source| ProjectIngestError::Io {
            path: dir.display().to_string(),
            source,
        })?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|source| ProjectIngestError::Io {
            path: path.display().to_string(),
            source,
        })?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if file_type.is_dir() {
            if ignored_project_dir(&name) {
                continue;
            }
            recursive_supported_files_inner(source_root, &path, scope, files)?;
        } else if file_type.is_file() {
            let rel = path
                .strip_prefix(source_root)
                .map_err(|source| ProjectIngestError::Io {
                    path: path.display().to_string(),
                    source: std::io::Error::new(std::io::ErrorKind::InvalidData, source),
                })?
                .to_string_lossy()
                .replace('\\', "/");
            if is_supported_project_file(&rel) && scope.includes_path(&rel) {
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

fn is_binary(bytes: &[u8]) -> bool {
    if bytes.contains(&0) {
        return true;
    }
    let control = bytes
        .iter()
        .filter(|byte| **byte < 0x20 && !matches!(**byte, b'\n' | b'\r' | b'\t'))
        .count();
    !bytes.is_empty() && control * 100 / bytes.len() > 20
}

fn validate_project_id(value: &str) -> Result<String, ProjectIngestError> {
    let valid = !value.is_empty()
        && value.len() <= 96
        && !value.contains("..")
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'));
    if valid {
        Ok(value.to_string())
    } else {
        Err(ProjectIngestError::InvalidProjectId {
            project_id: value.to_string(),
        })
    }
}

fn derive_project_id(repo_path: &Path) -> String {
    let name = repo_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("project");
    let slug = name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>();
    let digest = blake3_hex(repo_path.display().to_string().as_bytes());
    format!("{}-{}", slug.trim_matches('-'), &digest[..12])
}

fn cached_file_count(path: &Path) -> Result<usize, ProjectIngestError> {
    let conn = Connection::open(path).map_err(|source| ProjectIngestError::Sqlite {
        context: "open cache db",
        source,
    })?;
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM files", [], |row| row.get(0))
        .map_err(|source| ProjectIngestError::Sqlite {
            context: "count files",
            source,
        })?;
    Ok(count as usize)
}

fn load_cached_files(path: &Path) -> Result<Vec<CachedFileRow>, ProjectIngestError> {
    let conn = Connection::open(path).map_err(|source| ProjectIngestError::Sqlite {
        context: "open cache db",
        source,
    })?;
    let mut stmt = conn
        .prepare(
            "\
            SELECT f.path, f.blake3, f.size_bytes, COUNT(cp.chunk_key)
            FROM files f
            LEFT JOIN chunk_paths cp ON cp.path = f.path
            GROUP BY f.path, f.blake3, f.size_bytes
            ORDER BY f.path",
        )
        .map_err(|source| ProjectIngestError::Sqlite {
            context: "prepare cached files query",
            source,
        })?;
    let rows = stmt
        .query_map([], |row| {
            let path: String = row.get(0)?;
            Ok(CachedFileRow {
                role: project_role_for_path(&path),
                path,
                blake3: row.get(1)?,
                size_bytes: row.get::<_, i64>(2)? as u64,
                chunk_count: row.get::<_, i64>(3)? as usize,
            })
        })
        .map_err(|source| ProjectIngestError::Sqlite {
            context: "query cached files",
            source,
        })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|source| ProjectIngestError::Sqlite {
            context: "decode cached file row",
            source,
        })?);
    }
    Ok(out)
}

fn prediction_for_file(
    project_id: &str,
    repo_path: &Path,
    file: &CachedFileRow,
    merkle_root: &str,
) -> Result<RealityPrediction, ProjectIngestError> {
    let absolute = repo_path.join(&file.path);
    let bytes = fs::read(&absolute).map_err(|source| ProjectIngestError::Io {
        path: absolute.display().to_string(),
        source,
    })?;
    let text = decode_utf8_or_latin1(&bytes);
    let chunk_id = ChunkId(format!(
        "project_ingest:{project_id}:{}:{}",
        &file.blake3[..16],
        &blake3_hex(file.path.as_bytes())[..16]
    ));
    let failure_modes = division_by_zero_candidates(&text, &chunk_id);
    let security_concerns = security_concerns(&text, &chunk_id);
    let edge_cases = if failure_modes.is_empty() {
        Vec::new()
    } else {
        vec![PredictedEdgeCase {
            edge_class: EdgeCaseClass::BoundaryValue,
            chunk: chunk_id.clone(),
            line_range: failure_modes[0].line_range,
            triggering_input_description: "denominator value equals zero".to_string(),
            covered_by_test: false,
            confidence: 0.74,
        }]
    };
    let latent_bugs = if failure_modes.is_empty() {
        Vec::new()
    } else {
        vec![PredictedLatentBug {
            bug_class: LatentBugClass::InconsistentErrorHandling,
            chunk: chunk_id.clone(),
            line_range: failure_modes[0].line_range,
            confidence: 0.69,
            severity: Severity::High,
            explanation: "Division operation has no observable zero-denominator guard".to_string(),
        }]
    };
    let predicted_works = if failure_modes.is_empty() && security_concerns.is_empty() {
        vec![PredictedWorks {
            chunk: chunk_id.clone(),
            line_range: (1, line_count(&text)),
            claim:
                "project ingest parsed and cached this file without deterministic bug-pattern hits"
                    .to_string(),
            confidence: 0.62,
            supporting_embedders: vec![EmbedderId("E_AST".to_string())],
            similar_known_good_exemplars: Vec::new(),
            evidence_strength: 0.58,
        }]
    } else {
        Vec::new()
    };
    let prediction_id = project_prediction_id(project_id, &file.path, &file.blake3);
    let source_panel_sha = id32(&format!(
        "panel\0{project_id}\0{}\0{}\0{merkle_root}",
        file.path, file.blake3
    ));
    let task_hash = blake3_hex(format!("{}\0{}", file.path, file.blake3).as_bytes());
    let task_id = TaskId(format!("project_ingest:{project_id}:{}", &task_hash[..32]));
    RealityPredictionBuilder::from_parts(
        task_id,
        project_prediction_session_id(project_id),
        Language::Python,
        ConformalSet::try_new(vec![OracleOutcome::Abstain], 0.10, 0.50)?,
    )
    .prediction_id(prediction_id)
    .witness_hash(WitnessHash(id32(&format!(
        "witness\0{project_id}\0{}\0{}",
        file.path, file.blake3
    ))))
    .covered_chunks(vec![chunk_id])
    .verdict(Verdict::Abstain)
    .predicted_oracle_pass(if failure_modes.is_empty() { 0.50 } else { 0.25 })
    .predicted_test_pass(vec![if failure_modes.is_empty() { 0.50 } else { 0.20 }])
    .ood_score(0.35)
    .calibrated_confidence(if failure_modes.is_empty() { 0.41 } else { 0.66 })
    .degraded_status(true)
    .predicted_failure_modes(failure_modes)
    .predicted_security_concerns(security_concerns)
    .predicted_edge_cases(edge_cases)
    .predicted_latent_bugs(latent_bugs)
    .predicted_works(predicted_works)
    .predicted_reasoning_class(crate::types::ReasoningClass::Mute)
    .provenance(PredictionProvenance {
        predictor_version: PREDICTOR_VERSION.to_string(),
        constellation_version: "project-cache-merkle-v1".to_string(),
        calibration_version: "cold-cell-v1".to_string(),
        active_pointer: format!(
            "project={project_id};file_hash={};file_path_hash={}",
            file.blake3,
            &blake3_hex(file.path.as_bytes())[..16]
        ),
    })
    .source_panel_sha(source_panel_sha)
    .calibration_version("cold-cell-v1")
    .created_at_unix_ms(stable_prediction_timestamp(
        project_id,
        &file.path,
        &file.blake3,
    ))
    .build()
    .map_err(ProjectIngestError::Infer)
}

fn division_by_zero_candidates(text: &str, chunk: &ChunkId) -> Vec<PredictedFailureMode> {
    text.lines()
        .enumerate()
        .filter_map(|(idx, line)| {
            let trimmed = line.trim();
            let is_candidate = trimmed.contains('/')
                && !trimmed.contains("//")
                && !trimmed.contains(" if ")
                && !trimmed.contains("try:")
                && !trimmed.contains("ZeroDivisionError")
                && !trimmed.contains(" != 0")
                && !trimmed.contains("== 0");
            is_candidate.then(|| PredictedFailureMode {
                failure_class: FailureModeClass::Exception,
                chunk: chunk.clone(),
                line_range: ((idx + 1) as u32, (idx + 1) as u32),
                confidence: 0.76,
                severity: Severity::High,
                explanation: "possible DivisionByZero: division expression has no local zero-denominator guard"
                    .to_string(),
                contributing_embedders: vec![
                    EmbedderId("E_AST".to_string()),
                    EmbedderId("E_DataFlow".to_string()),
                ],
                root_cause_class: RootCauseClass::LogicError,
            })
        })
        .collect()
}

fn security_concerns(text: &str, chunk: &ChunkId) -> Vec<PredictedSecurityConcern> {
    text.lines()
        .enumerate()
        .filter_map(|(idx, line)| {
            let trimmed = line.trim();
            let explanation = if trimmed.contains("eval(") {
                Some("dynamic eval call in project source".to_string())
            } else if trimmed.contains("shell=True") {
                Some("shell=True command execution in project source".to_string())
            } else {
                None
            }?;
            Some(PredictedSecurityConcern {
                class: SecurityConcernClass::Other,
                chunk: chunk.clone(),
                line_range: ((idx + 1) as u32, (idx + 1) as u32),
                cvss_estimate: Some(7.5),
                explanation,
            })
        })
        .collect()
}

#[derive(Default)]
struct RoleCounts {
    source: usize,
    test: usize,
    doc: usize,
    config: usize,
}

fn role_counts(files: &[CachedFileRow]) -> RoleCounts {
    let mut counts = RoleCounts::default();
    for file in files {
        match file.role {
            ChunkRole::Source => counts.source += 1,
            ChunkRole::Test => counts.test += 1,
            ChunkRole::Doc => counts.doc += 1,
            ChunkRole::Config => counts.config += 1,
        }
    }
    counts
}

fn default_embedder_versions() -> Vec<EmbedderVersion> {
    [
        "e1", "e2", "e3", "e4", "e6", "e7", "e8", "e9", "e10", "e12", "e13", "e14",
    ]
    .into_iter()
    .map(|embedder| EmbedderVersion::new(embedder, "project-ingest-v1"))
    .collect()
}

fn decode_utf8_or_latin1(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(text) => text.to_string(),
        Err(_) => bytes.iter().map(|byte| *byte as char).collect(),
    }
}

fn line_count(text: &str) -> u32 {
    text.lines().count().max(1) as u32
}

fn id16(input: &str) -> [u8; 16] {
    let digest = blake3::hash(input.as_bytes());
    let mut out = [0u8; 16];
    out.copy_from_slice(&digest.as_bytes()[..16]);
    if out.iter().all(|byte| *byte == 0) {
        out[15] = 1;
    }
    out
}

fn id32(input: &str) -> [u8; 32] {
    *blake3::hash(input.as_bytes()).as_bytes()
}

fn stable_prediction_timestamp(project_id: &str, path: &str, file_blake3: &str) -> i64 {
    let digest = blake3::hash(format!("{project_id}\0{path}\0{file_blake3}").as_bytes());
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&digest.as_bytes()[..8]);
    1_700_000_000_000 + (u64::from_be_bytes(bytes) % 1_000_000_000) as i64
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}
