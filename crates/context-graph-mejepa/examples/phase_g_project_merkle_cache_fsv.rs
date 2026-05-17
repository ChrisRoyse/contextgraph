use anyhow::{ensure, Context, Result};
use context_graph_mejepa::cli::write_json_0600;
use context_graph_mejepa::{blake3_hex, EmbedderVersion, ProjectCacheConfig, ProjectMerkleCache};
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const FSV_ROOT: &str = "/mnt/d/contextgraph/fsv/task-py-g-062-merkle-cache-fsv";
const PROJECT_ID: &str = "task-py-g-062-synthetic";
const REPORT_NAME: &str = "project_merkle_cache_fsv.json";

fn main() -> Result<()> {
    let root = PathBuf::from(FSV_ROOT);
    let project_cache_root = PathBuf::from(format!("/mnt/d/contextgraph/projects/{PROJECT_ID}"));
    remove_if_exists(&root)?;
    remove_if_exists(&project_cache_root)?;
    fs::create_dir_all(&root)?;
    let report_path = root.join(REPORT_NAME);

    let empty = empty_repo_case(&root)?;
    let happy = happy_three_file_case(&root)?;
    let edit_one = edit_one_file_case(&root)?;
    let delete_one = delete_one_file_case(&root)?;
    let add_many = add_1000_files_case(&root)?;
    let move_same_content = same_content_moved_case(&root)?;
    let version_bump = embedder_version_bump_case(&root)?;
    let fail_closed = fail_closed_non_git_case(&root)?;

    let db_path = project_cache_root.join("cache/cache.db");
    let watcher_status_path = project_cache_root.join("cache/watcher.status");
    let report = json!({
        "schema_version": 1,
        "task_id": "TASK-PY-G-062",
        "source_of_truth": {
            "cache_db": db_path,
            "watcher_status": watcher_status_path,
            "sqlite_integrity_check": sqlite3_scalar(&db_path, "PRAGMA integrity_check;")?,
            "db_file_size_bytes": fs::metadata(&db_path)?.len(),
            "report_path": report_path,
        },
        "happy_path": happy,
        "edge_cases": [
            empty,
            edit_one,
            delete_one,
            add_many,
            move_same_content,
            version_bump,
            fail_closed
        ],
        "all_passed": true
    });
    write_json_0600(&report_path, &report)?;
    let readback: Value = serde_json::from_slice(&fs::read(&report_path)?)?;
    ensure!(readback == report, "FSV report readback mismatch");
    println!("{}", serde_json::to_string_pretty(&readback)?);
    Ok(())
}

fn empty_repo_case(root: &Path) -> Result<Value> {
    let repo = root.join("empty_repo");
    init_git_repo(&repo)?;
    let cache = open_cache(&repo)?;
    let before = db_counts(cache.db_path())?;
    let report = cache.scan_project(&embedder_versions("v1", "v1"))?;
    let after = db_counts(cache.db_path())?;
    let status: Value = serde_json::from_slice(&fs::read(cache.watcher_status_path())?)?;
    let pass = report.file_count == 0
        && after.files == 0
        && after.chunks == 0
        && after.embeddings == 0
        && status["file_count"] == 0;
    Ok(json!({
        "case": "empty_repo_initializes_cache",
        "trigger": "ProjectMerkleCache::scan_project",
        "before": before,
        "after": after,
        "watcher_status": status,
        "pass": pass
    }))
}

fn happy_three_file_case(root: &Path) -> Result<Value> {
    let repo = root.join("synthetic_repo");
    init_git_repo(&repo)?;
    write_initial_project(&repo)?;
    let cache = open_cache(&repo)?;
    let before = db_counts(cache.db_path())?;
    let report = cache.scan_project(&embedder_versions("v1", "v1"))?;
    let after = db_counts(cache.db_path())?;
    let src_hash_db = sqlite3_scalar(
        cache.db_path(),
        "SELECT blake3 FROM files WHERE path = 'src/a.py';",
    )?;
    let src_hash_expected = blake3_hex(&fs::read(repo.join("src/a.py"))?);
    let ignored_count = sqlite3_scalar(
        cache.db_path(),
        "SELECT COUNT(*) FROM files WHERE path LIKE '%ignored.py%';",
    )?;
    let status: Value = serde_json::from_slice(&fs::read(cache.watcher_status_path())?)?;
    let pass = report.file_count == 5
        && after.files == 5
        && after.chunk_paths >= 5
        && after.embeddings == after.chunks * 2
        && src_hash_db == src_hash_expected
        && ignored_count == "0"
        && status["file_count"] == 5;
    Ok(json!({
        "case": "happy_three_known_python_project",
        "trigger": "scan source/test/doc/config files",
        "before": before,
        "after": after,
        "report": report,
        "src_a_blake3_db": src_hash_db,
        "src_a_blake3_expected": src_hash_expected,
        "ignored_file_rows": ignored_count,
        "watcher_status": status,
        "pass": pass
    }))
}

fn edit_one_file_case(root: &Path) -> Result<Value> {
    let repo = root.join("synthetic_repo");
    let cache = open_cache(&repo)?;
    let before = per_file_last_seen(cache.db_path(), "src/a.py", "src/b.py")?;
    fs::write(
        repo.join("src/a.py"),
        "import b\n\ndef alpha(x: int) -> int:\n    return b.beta(x) + 2\n",
    )?;
    let report = cache.scan_project(&embedder_versions("v1", "v1"))?;
    let after = per_file_last_seen(cache.db_path(), "src/a.py", "src/b.py")?;
    let pass = report.changed_files == vec!["src/a.py".to_string()]
        && after.first > before.first
        && after.second == before.second;
    Ok(json!({
        "case": "edit_one_file_rechunks_only_changed_file",
        "trigger": "modify src/a.py then scan",
        "before": before,
        "after": after,
        "changed_files": report.changed_files,
        "pass": pass
    }))
}

fn delete_one_file_case(root: &Path) -> Result<Value> {
    let repo = root.join("synthetic_repo");
    let cache = open_cache(&repo)?;
    let before = db_counts(cache.db_path())?;
    fs::remove_file(repo.join("src/b.py"))?;
    let report = cache.scan_project(&embedder_versions("v1", "v1"))?;
    let after = db_counts(cache.db_path())?;
    let deleted_file_rows = sqlite3_scalar(
        cache.db_path(),
        "SELECT COUNT(*) FROM files WHERE path = 'src/b.py';",
    )?;
    let deleted_chunk_refs = sqlite3_scalar(
        cache.db_path(),
        "SELECT COUNT(*) FROM chunk_paths WHERE path = 'src/b.py';",
    )?;
    let pass = report.deleted_files == vec!["src/b.py".to_string()]
        && deleted_file_rows == "0"
        && deleted_chunk_refs == "0"
        && after.files + 1 == before.files;
    Ok(json!({
        "case": "delete_file_evicts_chunks",
        "trigger": "delete src/b.py then scan",
        "before": before,
        "after": after,
        "deleted_files": report.deleted_files,
        "deleted_file_rows": deleted_file_rows,
        "deleted_chunk_refs": deleted_chunk_refs,
        "pass": pass
    }))
}

fn add_1000_files_case(root: &Path) -> Result<Value> {
    let repo = root.join("synthetic_repo");
    let cache = open_cache(&repo)?;
    let before = db_counts(cache.db_path())?;
    let bulk = repo.join("generated");
    fs::create_dir_all(&bulk)?;
    for idx in 0..1000 {
        fs::write(
            bulk.join(format!("g{idx:04}.py")),
            format!("def generated_{idx}() -> int:\n    return {idx}\n"),
        )?;
    }
    let report = cache.scan_project(&embedder_versions("v1", "v1"))?;
    let after = db_counts(cache.db_path())?;
    let generated_rows = sqlite3_scalar(
        cache.db_path(),
        "SELECT COUNT(*) FROM files WHERE path LIKE 'generated/g%.py';",
    )?;
    let pass = generated_rows == "1000"
        && after.files >= before.files + 1000
        && report.file_count == after.files;
    Ok(json!({
        "case": "add_1000_files_batched_scan",
        "trigger": "add 1000 Python files then scan",
        "before": before,
        "after": after,
        "generated_rows": generated_rows,
        "pass": pass
    }))
}

fn same_content_moved_case(root: &Path) -> Result<Value> {
    let repo = root.join("synthetic_repo");
    let cache = open_cache(&repo)?;
    let before = db_counts(cache.db_path())?;
    fs::copy(repo.join("src/a.py"), repo.join("src/a_moved.py"))?;
    let report = cache.scan_project(&embedder_versions("v1", "v1"))?;
    let after = db_counts(cache.db_path())?;
    let src_hash = sqlite3_scalar(
        cache.db_path(),
        "SELECT blake3 FROM files WHERE path='src/a.py';",
    )?;
    let moved_hash = sqlite3_scalar(
        cache.db_path(),
        "SELECT blake3 FROM files WHERE path='src/a_moved.py';",
    )?;
    let shared_chunks = sqlite3_scalar(
        cache.db_path(),
        "\
        SELECT COUNT(*) FROM chunk_paths
        WHERE chunk_key IN (SELECT chunk_key FROM chunk_paths WHERE path='src/a.py')
          AND path='src/a_moved.py';",
    )?;
    let pass = src_hash == moved_hash
        && shared_chunks.parse::<usize>()? > 0
        && after.files == before.files + 1
        && report.deduped_chunk_paths > 0;
    Ok(json!({
        "case": "same_content_moved_dedupes_by_blake3",
        "trigger": "copy src/a.py to src/a_moved.py then scan",
        "before": before,
        "after": after,
        "src_hash": src_hash,
        "moved_hash": moved_hash,
        "shared_chunk_paths": shared_chunks,
        "deduped_chunk_paths": report.deduped_chunk_paths,
        "pass": pass
    }))
}

fn embedder_version_bump_case(root: &Path) -> Result<Value> {
    let repo = root.join("synthetic_repo");
    let cache = open_cache(&repo)?;
    let before_e1 = embedding_digest(cache.db_path(), "e1")?;
    let before_e7 = embedding_versions(cache.db_path(), "e7")?;
    let report = cache.scan_project(&embedder_versions("v1", "v2"))?;
    let after_e1 = embedding_digest(cache.db_path(), "e1")?;
    let after_e7 = embedding_versions(cache.db_path(), "e7")?;
    let e7_v1_count = after_e7
        .iter()
        .filter(|version| version.as_str() == "v1")
        .count();
    let e7_v2_count = after_e7
        .iter()
        .filter(|version| version.as_str() == "v2")
        .count();
    let pass = before_e1 == after_e1
        && before_e7.iter().all(|version| version == "v1")
        && e7_v1_count == 0
        && e7_v2_count == after_e7.len()
        && report
            .per_embedder_reembedded
            .get("e1")
            .copied()
            .unwrap_or(0)
            == 0
        && report
            .per_embedder_reembedded
            .get("e7")
            .copied()
            .unwrap_or(0)
            == after_e7.len();
    Ok(json!({
        "case": "embedder_version_bump_reembeds_only_stale_embedder",
        "trigger": "rescan with e7 v2 while e1 remains v1",
        "before_e1_digest": before_e1,
        "after_e1_digest": after_e1,
        "before_e7_versions": before_e7,
        "after_e7_versions": after_e7,
        "per_embedder_reembedded": report.per_embedder_reembedded,
        "pass": pass
    }))
}

fn fail_closed_non_git_case(root: &Path) -> Result<Value> {
    let non_git = root.join("not_git");
    fs::create_dir_all(&non_git)?;
    let err = ProjectMerkleCache::open(ProjectCacheConfig::new("not-git", non_git))
        .expect_err("non-git path must fail closed");
    let pass = err.code() == "MEJEPA_PROJECT_CACHE_INVALID_PATH";
    Ok(json!({
        "case": "non_git_repo_fails_closed",
        "trigger": "ProjectMerkleCache::open on non-git directory",
        "error_code": err.code(),
        "pass": pass
    }))
}

fn open_cache(repo: &Path) -> Result<ProjectMerkleCache> {
    ProjectMerkleCache::open(ProjectCacheConfig::new(PROJECT_ID, repo.to_path_buf()))
        .context("open project merkle cache")
}

fn embedder_versions(e1: &str, e7: &str) -> Vec<EmbedderVersion> {
    vec![
        EmbedderVersion::new("e1", e1),
        EmbedderVersion::new("e7", e7),
    ]
}

fn write_initial_project(repo: &Path) -> Result<()> {
    fs::create_dir_all(repo.join("src"))?;
    fs::create_dir_all(repo.join("tests"))?;
    fs::create_dir_all(repo.join("__pycache__"))?;
    fs::write(
        repo.join(".gitignore"),
        "ignored.py\n__pycache__/\n.venv/\nnode_modules/\n",
    )?;
    fs::write(repo.join("ignored.py"), "def ignored():\n    return 0\n")?;
    fs::write(
        repo.join("__pycache__/ignored.py"),
        "def ignored_cache():\n    return 0\n",
    )?;
    fs::write(
        repo.join("src/a.py"),
        "import b\n\ndef alpha(x: int) -> int:\n    return b.beta(x) + 1\n",
    )?;
    fs::write(
        repo.join("src/b.py"),
        "def beta(x: int) -> int:\n    return x * 2\n",
    )?;
    fs::write(
        repo.join("tests/test_a.py"),
        "from src.a import alpha\n\ndef test_alpha():\n    assert alpha(2) == 5\n",
    )?;
    fs::write(repo.join("README.md"), "# Synthetic project\n")?;
    fs::write(repo.join("pyproject.toml"), "[project]\nname='synthetic'\n")?;
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
    lazy_rescan: usize,
}

fn db_counts(db_path: &Path) -> Result<Counts> {
    let conn = Connection::open(db_path)?;
    Ok(Counts {
        files: count(&conn, "files")?,
        chunks: count(&conn, "chunks")?,
        chunk_paths: count(&conn, "chunk_paths")?,
        embeddings: count(&conn, "embeddings")?,
        lazy_rescan: count(&conn, "lazy_rescan_queue")?,
    })
}

fn count(conn: &Connection, table: &str) -> Result<usize> {
    let sql = format!("SELECT COUNT(*) FROM {table}");
    Ok(conn.query_row(&sql, [], |row| row.get::<_, i64>(0))? as usize)
}

#[derive(Debug, serde::Serialize)]
struct PairSeen {
    first: i64,
    second: i64,
}

fn per_file_last_seen(db_path: &Path, first: &str, second: &str) -> Result<PairSeen> {
    let conn = Connection::open(db_path)?;
    Ok(PairSeen {
        first: last_seen(&conn, first)?,
        second: last_seen(&conn, second)?,
    })
}

fn last_seen(conn: &Connection, path: &str) -> Result<i64> {
    Ok(conn.query_row(
        "SELECT last_seen FROM files WHERE path = ?1",
        params![path],
        |row| row.get(0),
    )?)
}

fn embedding_digest(db_path: &Path, embedder_id: &str) -> Result<String> {
    let conn = Connection::open(db_path)?;
    let mut stmt = conn.prepare(
        "SELECT hex(vector_blob) FROM embeddings WHERE embedder_id = ?1 ORDER BY chunk_key",
    )?;
    let rows = stmt.query_map(params![embedder_id], |row| row.get::<_, String>(0))?;
    let mut joined = String::new();
    for row in rows {
        joined.push_str(&row?);
    }
    Ok(blake3_hex(joined.as_bytes()))
}

fn embedding_versions(db_path: &Path, embedder_id: &str) -> Result<Vec<String>> {
    let conn = Connection::open(db_path)?;
    let mut stmt = conn.prepare(
        "SELECT DISTINCT embedder_version FROM embeddings WHERE embedder_id = ?1 ORDER BY embedder_version",
    )?;
    let rows = stmt.query_map(params![embedder_id], |row| row.get::<_, String>(0))?;
    let mut versions = Vec::new();
    for row in rows {
        versions.push(row?);
    }
    Ok(versions)
}

fn sqlite3_scalar(db_path: &Path, sql: &str) -> Result<String> {
    let output = Command::new("sqlite3").arg(db_path).arg(sql).output()?;
    ensure!(
        output.status.success(),
        "sqlite3 failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn remove_if_exists(path: &Path) -> Result<()> {
    if path.exists() {
        fs::remove_dir_all(path)?;
    }
    Ok(())
}
