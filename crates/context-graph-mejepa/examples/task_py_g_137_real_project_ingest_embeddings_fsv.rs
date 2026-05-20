use anyhow::{ensure, Context, Result};
use context_graph_core::types::fingerprint::{
    E10_DIM, E12_TOKEN_DIM, E13_SPLADE_VOCAB, E14_DIM, E1_DIM, E2_DIM, E3_DIM, E4_DIM,
    E6_SPARSE_VOCAB, E7_DIM, E8_DIM, E9_DIM,
};
use context_graph_mejepa::{
    run_project_ingest_with_multi_array_provider, write_json_0600, ProjectIngestMode,
    ProjectIngestRequest, ProjectIngestScope,
};
use rusqlite::Connection;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

const FSV_ROOT: &str =
    "/zfs/archive/contextgraph/fsv/task-py-g-137-real-project-ingest-embeddings-fsv";
const PROJECT_ID: &str = "task-py-g-137-real-project-ingest-embeddings";
const REPORT_NAME: &str = "project_ingest_real_embeddings_fsv.json";

#[tokio::main]
async fn main() -> Result<()> {
    let root = PathBuf::from(FSV_ROOT).join(format!(
        "run-{}",
        chrono::Utc::now().format("%Y%m%dT%H%M%SZ")
    ));
    fs::create_dir_all(&root)?;

    let repo = root.join("python_project");
    fs::create_dir_all(repo.join("src"))?;
    fs::write(
        repo.join("src/calculator.py"),
        "\
def divide(a: float, b: float) -> float:
    if b == 0:
        raise ValueError('division by zero')
    return a / b


def moving_average(values):
    if not values:
        return 0.0
    return sum(values) / len(values)
",
    )?;

    reset_project(PROJECT_ID)?;
    let models_dir = models_dir();
    context_graph_embeddings::initialize_global_warm_provider_with_models_dir(&models_dir)
        .await
        .with_context(|| format!("warm production provider from {}", models_dir.display()))?;
    let provider = context_graph_embeddings::get_warm_provider()
        .context("read initialized production multi-array provider")?;
    ensure!(provider.is_ready(), "production provider is not ready");

    let report = run_project_ingest_with_multi_array_provider(
        ProjectIngestRequest {
            repo_path: repo.clone(),
            project_id: Some(PROJECT_ID.to_string()),
            mode: ProjectIngestMode::Full,
            scope: ProjectIngestScope::SourceOnly,
            overwrite: true,
            changed_paths: Vec::new(),
        },
        provider.clone(),
    )
    .await?;
    let sqlite_rows = read_embedding_rows(Path::new(&report.cache_db_path))?;
    let chunk_count = count_rows(Path::new(&report.cache_db_path), "chunks")?;
    let embedding_count = count_rows(Path::new(&report.cache_db_path), "embeddings")?;
    let expected_embedders = expected_embedder_shapes()
        .keys()
        .map(|value| (*value).to_string())
        .collect::<BTreeSet<_>>();
    let observed_embedders = sqlite_rows
        .iter()
        .map(|row| row.embedder_id.clone())
        .collect::<BTreeSet<_>>();
    let rows_per_embedder = rows_per_embedder(&sqlite_rows);
    let all_rows_valid = sqlite_rows.iter().all(validate_row);

    ensure!(chunk_count > 0, "project ingest produced no AST chunks");
    ensure!(
        embedding_count as usize == sqlite_rows.len(),
        "embedding row count readback mismatch"
    );
    ensure!(
        observed_embedders == expected_embedders,
        "observed embedder set did not match expected active project-ingest embedders"
    );
    ensure!(all_rows_valid, "at least one embedding row was invalid");
    ensure!(
        rows_per_embedder
            .values()
            .all(|count| *count == chunk_count as usize),
        "each active embedder must have one row per chunk"
    );

    let report_path = root.join(REPORT_NAME);
    let evidence = json!({
        "schemaVersion": 1,
        "taskId": "TASK-PY-G-137",
        "issue": 437,
        "modelsDir": models_dir,
        "projectId": PROJECT_ID,
        "sourceOfTruth": {
            "repo": repo,
            "cacheSqlite": report.cache_db_path,
            "manifest": report.manifest_path,
            "fsvReport": report_path
        },
        "provider": {
            "ready": provider.is_ready(),
            "health": provider.health_status(),
            "modelIds": provider.model_ids()
        },
        "ingest": {
            "filesAfter": report.files_after,
            "predictionsWritten": report.predictions_written,
            "newEmbeddingsWritten": report.new_embeddings_written,
            "cachePerEmbedderReembedded": report.cache_report.per_embedder_reembedded,
            "changedFiles": report.cache_report.changed_files
        },
        "sqliteReadback": {
            "chunkCount": chunk_count,
            "embeddingCount": embedding_count,
            "observedEmbedders": observed_embedders,
            "rowsPerEmbedder": rows_per_embedder,
            "sampleRows": sqlite_rows.iter().take(24).collect::<Vec<_>>(),
            "allRowsValid": all_rows_valid
        },
        "allPassed": true
    });
    write_json_0600(&report_path, &evidence)?;
    let readback: Value = serde_json::from_slice(&fs::read(&report_path)?)?;
    ensure!(readback == evidence, "FSV JSON readback mismatch");
    println!("{}", serde_json::to_string_pretty(&readback)?);
    Ok(())
}

fn models_dir() -> PathBuf {
    std::env::var("CONTEXT_GRAPH_MODELS_PATH")
        .or_else(|_| std::env::var("CONTEXTGRAPH_MODELS_ROOT"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/zfs/hot/contextgraph/models"))
}

fn reset_project(project_id: &str) -> Result<()> {
    let path = Path::new("/zfs/archive/contextgraph/projects").join(project_id);
    if path.exists() {
        fs::remove_dir_all(&path)
            .with_context(|| format!("remove stale project {}", path.display()))?;
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
struct EmbeddingReadback {
    chunk_key: String,
    embedder_id: String,
    embedder_version: String,
    vector_len: usize,
    vector_format: String,
    dimension: u32,
    model_id: String,
    precision_class: String,
    output_sha256_len: usize,
    provenance: String,
}

fn read_embedding_rows(db_path: &Path) -> Result<Vec<EmbeddingReadback>> {
    let conn = Connection::open(db_path)
        .with_context(|| format!("open cache SQLite {}", db_path.display()))?;
    let mut stmt = conn.prepare(
        "\
        SELECT chunk_key,
               embedder_id,
               embedder_version,
               length(vector_blob),
               vector_format,
               dimension,
               model_id,
               precision_class,
               length(output_sha256),
               provenance
        FROM embeddings
        ORDER BY chunk_key, embedder_id, embedder_version",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(EmbeddingReadback {
            chunk_key: row.get(0)?,
            embedder_id: row.get(1)?,
            embedder_version: row.get(2)?,
            vector_len: row.get::<_, i64>(3)? as usize,
            vector_format: row.get(4)?,
            dimension: row.get::<_, i64>(5)? as u32,
            model_id: row.get(6)?,
            precision_class: row.get(7)?,
            output_sha256_len: row.get::<_, i64>(8)? as usize,
            provenance: row.get(9)?,
        })
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .context("collect embedding readback rows")
}

fn count_rows(db_path: &Path, table: &str) -> Result<i64> {
    ensure!(
        matches!(table, "chunks" | "embeddings"),
        "unsupported readback table"
    );
    let conn = Connection::open(db_path)
        .with_context(|| format!("open cache SQLite {}", db_path.display()))?;
    conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
        row.get(0)
    })
    .context("count readback rows")
}

fn rows_per_embedder(rows: &[EmbeddingReadback]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for row in rows {
        *counts.entry(row.embedder_id.clone()).or_insert(0) += 1;
    }
    counts
}

fn validate_row(row: &EmbeddingReadback) -> bool {
    let shapes = expected_embedder_shapes();
    let Some((format, dimension)) = shapes.get(row.embedder_id.as_str()) else {
        return false;
    };
    row.vector_len != 32
        && row.vector_len > 0
        && row.vector_format == *format
        && row.dimension == *dimension
        && row.embedder_version == "project-ingest-v1"
        && !row.model_id.trim().is_empty()
        && row.precision_class == "multi_array_provider_slot_preserving"
        && row.output_sha256_len == 64
        && row
            .provenance
            .contains("\"producer\":\"MultiArrayEmbeddingProvider.embed_batch_all\"")
}

fn expected_embedder_shapes() -> BTreeMap<&'static str, (&'static str, u32)> {
    BTreeMap::from([
        ("e1", ("dense_f32_le", E1_DIM as u32)),
        ("e2", ("dense_f32_le", E2_DIM as u32)),
        ("e3", ("dense_f32_le", E3_DIM as u32)),
        ("e4", ("dense_f32_le", E4_DIM as u32)),
        ("e6", ("sparse_json", E6_SPARSE_VOCAB as u32)),
        ("e7", ("dense_f32_le", E7_DIM as u32)),
        ("e8", ("dense_f32_le", E8_DIM as u32)),
        ("e9", ("dense_f32_le", E9_DIM as u32)),
        ("e10", ("dense_f32_le", E10_DIM as u32)),
        ("e12", ("token_f32_json", E12_TOKEN_DIM as u32)),
        ("e13", ("sparse_json", E13_SPLADE_VOCAB as u32)),
        ("e14", ("dense_f32_le", E14_DIM as u32)),
    ])
}
