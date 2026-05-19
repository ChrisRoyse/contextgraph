use anyhow::{ensure, Context, Result};
use context_graph_mejepa::cli::write_json_0600;
use context_graph_mejepa::{
    EmbedderVersion, ProjectCacheConfig, ProjectCacheError, ProjectMerkleCache,
};
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const DEFAULT_FSV_ROOT: &str =
    "/zfs/archive/contextgraph/fsv/task-py-g-123-project-cache-no-fake-embeddings-fsv";
const PROJECT_ID: &str = "task-py-g-123-no-fake-embeddings";
const REPORT_NAME: &str = "project_cache_no_fake_embeddings_fsv.json";

fn main() -> Result<()> {
    let root = std::env::var("CG_MEJEPA_TASK_PY_G_123_FSV_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(DEFAULT_FSV_ROOT));
    remove_if_exists(&root)?;
    fs::create_dir_all(&root)?;

    let project_root = ProjectCacheConfig::new(PROJECT_ID, PathBuf::new())
        .cache_dir
        .parent()
        .context("project root missing from cache dir")?
        .to_path_buf();
    remove_if_exists(&project_root)?;

    let repo = root.join("source_repo");
    init_git_repo(&repo)?;
    fs::create_dir_all(repo.join("src"))?;
    fs::write(
        repo.join("src/reality.py"),
        "def answer(value: int) -> int:\n    return value + 1\n",
    )?;

    let cache = ProjectMerkleCache::open(ProjectCacheConfig::new(PROJECT_ID, repo.clone()))?;
    let cache_only_report = cache.scan_project(&[])?;
    let cache_only_counts = db_counts(cache.db_path())?;

    fs::write(
        repo.join("src/reality.py"),
        "def answer(value: int) -> int:\n    return value + 2\n",
    )?;
    let fail_closed = match cache.scan_project(&[EmbedderVersion::new("e1", "project-ingest-v1")]) {
        Ok(report) => json!({
            "passed": false,
            "unexpected_report": report
        }),
        Err(err) => json!({
            "passed": err.code() == "MEJEPA_PROJECT_CACHE_REAL_EMBEDDING_REQUIRED",
            "code": err.code(),
            "message": err.to_string()
        }),
    };

    let post_fail_counts = db_counts(cache.db_path())?;
    insert_legacy_32_byte_embedding(cache.db_path())?;
    let legacy_before_reopen = count_32_byte_embeddings(cache.db_path())?;
    let reopened = ProjectMerkleCache::open(ProjectCacheConfig::new(PROJECT_ID, repo))?;
    let legacy_after_reopen = count_32_byte_embeddings(reopened.db_path())?;
    let source = fs::read_to_string("crates/context-graph-mejepa/src/project_cache.rs")?;
    let source_has_legacy_writer = source.contains("fn deterministic_vector_blob")
        || source.contains("blake3::hash(format!(\"{chunk_key}\\0{embedder_id}\\0{version}\")");

    let all_passed = cache_only_report.file_count == 1
        && cache_only_report.unique_chunk_count > 0
        && cache_only_counts.embeddings == 0
        && fail_closed
            .get("passed")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        && post_fail_counts.embeddings == 0
        && legacy_before_reopen == 1
        && legacy_after_reopen == 0
        && !source_has_legacy_writer;

    let report_path = root.join(REPORT_NAME);
    let report = json!({
        "schema_version": 1,
        "task_id": "TASK-PY-G-123",
        "issue": 424,
        "source_of_truth": {
            "cache_db": reopened.db_path(),
            "watcher_status": reopened.watcher_status_path(),
            "report_path": report_path,
        },
        "cache_only_scan": {
            "report": cache_only_report,
            "counts": cache_only_counts,
        },
        "embedder_requested_fail_closed": fail_closed,
        "post_fail_counts": post_fail_counts,
        "legacy_synthetic_purge": {
            "inserted_32_byte_rows": legacy_before_reopen,
            "rows_after_reopen": legacy_after_reopen,
        },
        "source_has_legacy_writer": source_has_legacy_writer,
        "all_passed": all_passed,
    });
    write_json_0600(&report_path, &report)?;
    let readback: Value = serde_json::from_slice(&fs::read(&report_path)?)?;
    ensure!(readback == report, "FSV report readback mismatch");
    ensure!(all_passed, "project-cache no-fake-embeddings FSV failed");
    println!("{}", serde_json::to_string_pretty(&readback)?);
    Ok(())
}

fn init_git_repo(repo: &Path) -> Result<()> {
    fs::create_dir_all(repo)?;
    run(Command::new("git").arg("init").arg("-q").current_dir(repo))?;
    run(Command::new("git")
        .args(["config", "user.email", "fsv@example.invalid"])
        .current_dir(repo))?;
    run(Command::new("git")
        .args(["config", "user.name", "FSV"])
        .current_dir(repo))?;
    Ok(())
}

fn run(command: &mut Command) -> Result<()> {
    let output = command.output()?;
    ensure!(
        output.status.success(),
        "command failed: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}

#[derive(Debug, serde::Serialize)]
struct Counts {
    files: usize,
    chunks: usize,
    chunk_paths: usize,
    embeddings: usize,
    legacy_32_byte_embeddings: usize,
}

fn db_counts(db_path: &Path) -> Result<Counts> {
    let conn = Connection::open(db_path)?;
    Ok(Counts {
        files: count(&conn, "files")?,
        chunks: count(&conn, "chunks")?,
        chunk_paths: count(&conn, "chunk_paths")?,
        embeddings: count(&conn, "embeddings")?,
        legacy_32_byte_embeddings: count_32_byte_embeddings(db_path)?,
    })
}

fn count(conn: &Connection, table: &str) -> Result<usize> {
    let sql = format!("SELECT COUNT(*) FROM {table}");
    Ok(conn.query_row(&sql, [], |row| row.get::<_, i64>(0))? as usize)
}

fn count_32_byte_embeddings(db_path: &Path) -> Result<usize> {
    let conn = Connection::open(db_path)?;
    Ok(conn.query_row(
        "SELECT COUNT(*) FROM embeddings WHERE length(vector_blob) = 32",
        [],
        |row| row.get::<_, i64>(0),
    )? as usize)
}

fn insert_legacy_32_byte_embedding(db_path: &Path) -> Result<()> {
    let conn = Connection::open(db_path)?;
    let chunk_key: String = conn.query_row(
        "SELECT chunk_key FROM chunks ORDER BY chunk_key LIMIT 1",
        [],
        |row| row.get(0),
    )?;
    conn.execute(
        "\
        INSERT OR REPLACE INTO embeddings(
            chunk_key, embedder_id, embedder_version, vector_blob, updated_at
        ) VALUES (?1, 'e1', 'project-ingest-v1', ?2, 1)",
        params![chunk_key, vec![9_u8; 32]],
    )?;
    Ok(())
}

fn remove_if_exists(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    ensure!(
        path.starts_with("/zfs/archive/contextgraph/fsv/")
            || path.starts_with("/zfs/archive/contextgraph/projects/"),
        "refusing to remove path outside aiwonder contextgraph roots: {}",
        path.display()
    );
    fs::remove_dir_all(path).map_err(|source| ProjectCacheError::Io {
        path: path.display().to_string(),
        source,
    })?;
    Ok(())
}
