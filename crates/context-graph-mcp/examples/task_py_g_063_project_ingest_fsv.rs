use anyhow::{ensure, Context, Result};
use async_trait::async_trait;
use context_graph_core::monitoring::StubLayerStatusProvider;
use context_graph_core::traits::{
    EmbeddingMetadata, MultiArrayEmbeddingOutput, MultiArrayEmbeddingProvider,
    TeleologicalMemoryStore,
};
use context_graph_core::types::fingerprint::NUM_EMBEDDERS;
use context_graph_core::{CoreError, CoreResult};
use context_graph_mcp::handlers::Handlers;
use context_graph_mcp::protocol::{JsonRpcId, JsonRpcRequest};
use context_graph_mejepa::{
    count_project_predictions, open_infer_rocksdb, project_prediction_live_key, run_project_ingest,
    ProjectIngestManifest, ProjectIngestMode, ProjectIngestRequest, ProjectIngestScope,
    RealityPrediction,
};
use context_graph_mejepa_cf::CF_MEJEPA_LIVE_PREDICTIONS;
use context_graph_storage::teleological::RocksDbTeleologicalStore;
use context_graph_storage::{BackgroundGraphBuilder, EdgeRepository, GraphBuilderConfig};
use rocksdb::IteratorMode;
use rusqlite::Connection;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

const FSV_ROOT: &str = "/mnt/d/contextgraph/fsv/task-py-g-063-project-ingest-fsv";
const REPORT_NAME: &str = "project_ingest_fsv.json";

fn main() -> Result<()> {
    let root = PathBuf::from(FSV_ROOT).join(format!(
        "run-{}",
        chrono::Utc::now().format("%Y%m%dT%H%M%SZ")
    ));
    remove_if_exists(&root)?;
    fs::create_dir_all(&root)?;
    let mcp_surface = mcp_surface_case(&root)?;
    let happy = happy_path_case(&root)?;
    let empty = empty_repo_case(&root)?;
    let incremental = unchanged_incremental_case(&root)?;
    let large = large_repo_case(&root)?;
    let mixed = mixed_encoding_binary_case(&root)?;
    let scope_switch = scope_switch_case(&root)?;
    let plain_source = plain_directory_and_single_file_case(&root)?;
    let edge_cases = vec![empty, incremental, large, mixed, scope_switch, plain_source];
    let all_passed =
        case_pass(&mcp_surface) && case_pass(&happy) && edge_cases.iter().all(case_pass);
    let report_path = root.join(REPORT_NAME);
    let report = json!({
        "schemaVersion": 1,
        "taskId": "TASK-PY-G-063",
        "issue": 282,
        "sourceOfTruth": {
            "projectManifests": "/mnt/d/contextgraph/projects/<project_id>/manifest.json",
            "cacheSqlite": "/mnt/d/contextgraph/projects/<project_id>/cache/cache.db",
            "livePredictionCf": CF_MEJEPA_LIVE_PREDICTIONS,
            "fsvReport": report_path
        },
        "mcpSurface": mcp_surface,
        "happyPath": happy,
        "edgeCases": edge_cases,
        "allPassed": all_passed
    });
    write_json_0600(&report_path, &report)?;
    let readback: Value = serde_json::from_slice(&fs::read(&report_path)?)?;
    ensure!(readback == report, "FSV report readback mismatch");
    ensure!(all_passed, "one or more project ingest FSV cases failed");
    println!("{}", serde_json::to_string_pretty(&readback)?);
    Ok(())
}

fn mcp_surface_case(root: &Path) -> Result<Value> {
    let defs = context_graph_mcp::tools::get_tool_definitions();
    let canonical = context_graph_mcp::tools::names::MEJEPA_PROJECT_INGEST;
    let tool = defs.iter().find(|tool| tool.name == canonical);
    let alias = context_graph_mcp::tools::aliases::resolve_alias("mejepa_project_ingest");
    let canonical_project_id = "task-py-g-063-mcp-canonical";
    let alias_project_id = "task-py-g-063-mcp-alias";
    reset_project(canonical_project_id)?;
    reset_project(alias_project_id)?;
    let canonical_repo = root.join("mcp_canonical_repo");
    let alias_repo = root.join("mcp_alias_repo");
    init_plain_project_dir(&canonical_repo)?;
    init_plain_project_dir(&alias_repo)?;
    fs::write(
        canonical_repo.join("direct.py"),
        "def direct(a, b):\n    return a + b\n",
    )?;
    fs::write(
        alias_repo.join("alias.py"),
        "def alias(a, b):\n    return a / b\n",
    )?;
    let canonical_dispatch =
        call_project_ingest_via_mcp(root, canonical, &canonical_repo, canonical_project_id)?;
    let alias_dispatch =
        call_project_ingest_via_mcp(root, "mejepa_project_ingest", &alias_repo, alias_project_id)?;
    let canonical_after = inspect_state(canonical_project_id)?;
    let alias_after = inspect_state(alias_project_id)?;
    let pass = tool.is_some()
        && alias == canonical
        && tool
            .and_then(|tool| tool.input_schema.pointer("/properties/mode/enum"))
            .and_then(Value::as_array)
            .is_some_and(|values| values.iter().any(|value| value == "Incremental"))
        && tool
            .and_then(|tool| tool.input_schema.pointer("/properties/dbPath"))
            .is_none()
        && canonical_after.project_prediction_rows == 1
        && alias_after.project_prediction_rows == 1
        && canonical_dispatch
            .get("projectId")
            .and_then(Value::as_str)
            .is_some_and(|value| value == canonical_project_id)
        && alias_dispatch
            .get("projectId")
            .and_then(Value::as_str)
            .is_some_and(|value| value == alias_project_id);
    Ok(json!({
        "case": "mcp_tool_definition_and_alias",
        "before": {"toolPresent": false},
        "trigger": "tools::get_tool_definitions + aliases::resolve_alias + tools/call canonical and alias dispatch",
        "after": {
            "toolPresent": tool.is_some(),
            "alias": alias,
            "canonical": canonical,
            "schema": tool.map(|tool| tool.input_schema.clone()),
            "canonicalDispatch": canonical_dispatch,
            "aliasDispatch": alias_dispatch,
            "canonicalState": canonical_after,
            "aliasState": alias_after
        },
        "pass": pass
    }))
}

fn happy_path_case(root: &Path) -> Result<Value> {
    let project_id = "task-py-g-063-happy";
    reset_project(project_id)?;
    let repo = root.join("happy_repo");
    init_git_repo(&repo)?;
    fs::create_dir_all(repo.join("src"))?;
    fs::write(
        repo.join("src/clean.py"),
        "def add(a: int, b: int) -> int:\n    return a + b\n",
    )?;
    fs::write(
        repo.join("src/bug.py"),
        "def divide(a, b):\n    return a / b\n",
    )?;
    fs::write(
        repo.join("src/security.py"),
        "def run(expr):\n    return eval(expr)\n",
    )?;
    let before = inspect_state(project_id)?;
    let report = run_project_ingest(ProjectIngestRequest {
        repo_path: repo.clone(),
        project_id: Some(project_id.to_string()),
        mode: ProjectIngestMode::Full,
        scope: ProjectIngestScope::SourceOnly,
        overwrite: false,
    })?;
    let after = inspect_state(project_id)?;
    let predictions = read_project_predictions(&default_db_path(project_id)?, project_id)?;
    let physical_keys =
        verify_manifest_physical_keys(&default_db_path(project_id)?, project_id, &report.manifest)?;
    let bug = prediction_by_file(&report.manifest, &predictions, "src/bug.py")?;
    let clean = prediction_by_file(&report.manifest, &predictions, "src/clean.py")?;
    let security = prediction_by_file(&report.manifest, &predictions, "src/security.py")?;
    let pass = !before.manifest_exists
        && after
            .manifest
            .as_ref()
            .is_some_and(|manifest| manifest.file_count == 3)
        && after.sqlite.files == 3
        && after.project_prediction_rows == 3
        && report.predictions_written == 3
        && bug
            .predicted_failure_modes
            .iter()
            .any(|mode| mode.confidence > 0.5 && mode.explanation.contains("DivisionByZero"))
        && !clean.predicted_works.is_empty()
        && security.predicted_security_concerns.len() == 1;
    Ok(json!({
        "case": "happy_three_python_files",
        "before": before,
        "trigger": "Full SourceOnly ingest on clean, DivisionByZero, eval files",
        "after": after,
        "reportSummary": summarize_report(&report),
        "directPredictionReadback": {
            "rows": predictions.len(),
            "physicalKeys": physical_keys,
            "bugFailureModes": bug.predicted_failure_modes,
            "cleanPredictedWorks": clean.predicted_works,
            "securityConcerns": security.predicted_security_concerns
        },
        "pass": pass
    }))
}

fn empty_repo_case(root: &Path) -> Result<Value> {
    let project_id = "task-py-g-063-empty";
    reset_project(project_id)?;
    let repo = root.join("empty_repo");
    init_git_repo(&repo)?;
    fs::write(repo.join("README.md"), "# no python here\n")?;
    let before = inspect_state(project_id)?;
    let report = run_project_ingest(ProjectIngestRequest {
        repo_path: repo,
        project_id: Some(project_id.to_string()),
        mode: ProjectIngestMode::Full,
        scope: ProjectIngestScope::SourceOnly,
        overwrite: false,
    })?;
    let after = inspect_state(project_id)?;
    let pass = report.manifest.file_count == 0
        && report.manifest.prediction_count == 0
        && report.project_prediction_rows_after == 0
        && after.sqlite.files == 0;
    Ok(json!({
        "case": "zero_python_files_writes_empty_manifest",
        "before": before,
        "trigger": "Full SourceOnly ingest on repo with only README.md",
        "after": after,
        "reportSummary": summarize_report(&report),
        "pass": pass
    }))
}

fn unchanged_incremental_case(root: &Path) -> Result<Value> {
    let project_id = "task-py-g-063-incremental";
    reset_project(project_id)?;
    let repo = root.join("incremental_repo");
    init_git_repo(&repo)?;
    fs::create_dir_all(repo.join("pkg"))?;
    fs::write(repo.join("pkg/a.py"), "def a():\n    return 1\n")?;
    let first = run_project_ingest(ProjectIngestRequest {
        repo_path: repo.clone(),
        project_id: Some(project_id.to_string()),
        mode: ProjectIngestMode::Full,
        scope: ProjectIngestScope::SourceOnly,
        overwrite: false,
    })?;
    thread::sleep(Duration::from_millis(5));
    let chunk_digest_before =
        chunk_updated_at_digest(&project_root(project_id).join("cache/cache.db"))?;
    let before = inspect_state(project_id)?;
    let second = run_project_ingest(ProjectIngestRequest {
        repo_path: repo,
        project_id: Some(project_id.to_string()),
        mode: ProjectIngestMode::Incremental,
        scope: ProjectIngestScope::SourceOnly,
        overwrite: false,
    })?;
    let after = inspect_state(project_id)?;
    let chunk_digest_after =
        chunk_updated_at_digest(&project_root(project_id).join("cache/cache.db"))?;
    let before_ts = first.manifest.last_ingest_unix_ms;
    let after_ts = second.manifest.last_ingest_unix_ms;
    let pass = before.project_prediction_rows == 1
        && after.project_prediction_rows == 1
        && second.predictions_written == 0
        && second.new_embeddings_written == 0
        && second.manifest.prediction_count == 1
        && chunk_digest_before == chunk_digest_after
        && after_ts > before_ts;
    Ok(json!({
        "case": "unchanged_incremental_updates_manifest_only",
        "before": before,
        "trigger": "Incremental SourceOnly ingest with no file changes",
        "after": after,
        "firstReportSummary": summarize_report(&first),
        "secondReportSummary": summarize_report(&second),
        "timestampBefore": before_ts,
        "timestampAfter": after_ts,
        "chunkUpdatedAtDigestBefore": chunk_digest_before,
        "chunkUpdatedAtDigestAfter": chunk_digest_after,
        "pass": pass
    }))
}

fn large_repo_case(root: &Path) -> Result<Value> {
    let project_id = "task-py-g-063-large";
    reset_project(project_id)?;
    let repo = root.join("large_repo");
    init_git_repo(&repo)?;
    let src = repo.join("src");
    fs::create_dir_all(&src)?;
    for idx in 0..5001 {
        fs::write(
            src.join(format!("m{idx:04}.py")),
            format!("def f_{idx}():\n    return {idx}\n"),
        )?;
    }
    let before = inspect_state(project_id)?;
    let report = run_project_ingest(ProjectIngestRequest {
        repo_path: repo,
        project_id: Some(project_id.to_string()),
        mode: ProjectIngestMode::Full,
        scope: ProjectIngestScope::SourceOnly,
        overwrite: false,
    })?;
    let after = inspect_state(project_id)?;
    let pass = report.manifest.file_count == 5001
        && report.project_prediction_rows_after == 5001
        && after.sqlite.files == 5001
        && after
            .manifest
            .as_ref()
            .is_some_and(|manifest| manifest.prediction_count == 5001);
    Ok(json!({
        "case": "large_repo_over_5000_files",
        "before": before,
        "trigger": "Full SourceOnly ingest on 5001 one-function Python files",
        "after": after,
        "reportSummary": summarize_report(&report),
        "pass": pass
    }))
}

fn mixed_encoding_binary_case(root: &Path) -> Result<Value> {
    let project_id = "task-py-g-063-mixed";
    reset_project(project_id)?;
    let repo = root.join("mixed_repo");
    init_git_repo(&repo)?;
    fs::create_dir_all(repo.join("pkg"))?;
    fs::write(
        repo.join("pkg/utf8.py"),
        "def hello():\n    return 'hello'\n",
    )?;
    fs::write(
        repo.join("pkg/latin1.py"),
        b"# coding: latin-1\nNAME = 'caf\xe9'\ndef name():\n    return NAME\n",
    )?;
    let before = inspect_state(project_id)?;
    let ok_report = run_project_ingest(ProjectIngestRequest {
        repo_path: repo.clone(),
        project_id: Some(project_id.to_string()),
        mode: ProjectIngestMode::Full,
        scope: ProjectIngestScope::SourceOnly,
        overwrite: false,
    })?;
    let after_text = inspect_state(project_id)?;
    fs::write(repo.join("pkg/binary.py"), [0, 1, 2, 3, 255])?;
    let binary_err = run_project_ingest(ProjectIngestRequest {
        repo_path: repo,
        project_id: Some(project_id.to_string()),
        mode: ProjectIngestMode::Incremental,
        scope: ProjectIngestScope::SourceOnly,
        overwrite: false,
    })
    .expect_err("binary scoped file must fail closed");
    let after_binary = inspect_state(project_id)?;
    let pass = ok_report.manifest.file_count == 2
        && after_text.project_prediction_rows == 2
        && binary_err.code() == "MEJEPA_PROJECT_INGEST_BINARY_REJECTED"
        && after_binary.project_prediction_rows == 2;
    Ok(json!({
        "case": "mixed_utf8_latin1_then_binary_fail_closed",
        "before": before,
        "trigger": "Full ingest of UTF-8+Latin-1, then Incremental after adding binary .py",
        "afterTextIngest": after_text,
        "afterBinaryAttempt": after_binary,
        "textReportSummary": summarize_report(&ok_report),
        "binaryErrorCode": binary_err.code(),
        "pass": pass
    }))
}

fn scope_switch_case(root: &Path) -> Result<Value> {
    let project_id = "task-py-g-063-scope";
    reset_project(project_id)?;
    let repo = root.join("scope_repo");
    init_git_repo(&repo)?;
    fs::create_dir_all(repo.join("src"))?;
    fs::create_dir_all(repo.join("tests"))?;
    fs::write(repo.join("src/app.py"), "def value():\n    return 7\n")?;
    fs::write(
        repo.join("src/helper_test.py"),
        "def test_helper_shape():\n    return True\n",
    )?;
    fs::write(
        repo.join("tests/test_app.py"),
        "from src.app import value\n\ndef test_value():\n    assert value() == 7\n",
    )?;
    let source_only = run_project_ingest(ProjectIngestRequest {
        repo_path: repo.clone(),
        project_id: Some(project_id.to_string()),
        mode: ProjectIngestMode::Full,
        scope: ProjectIngestScope::SourceOnly,
        overwrite: false,
    })?;
    let before = inspect_state(project_id)?;
    let source_prediction_id = source_only.manifest.predictions[0]
        .prediction_id_hex
        .clone();
    let source_and_tests = run_project_ingest(ProjectIngestRequest {
        repo_path: repo,
        project_id: Some(project_id.to_string()),
        mode: ProjectIngestMode::Incremental,
        scope: ProjectIngestScope::SourceAndTests,
        overwrite: false,
    })?;
    let after = inspect_state(project_id)?;
    let source_after = source_and_tests
        .manifest
        .predictions
        .iter()
        .find(|row| row.file_path == "src/app.py")
        .context("source row missing after scope switch")?;
    let pass = before
        .manifest
        .as_ref()
        .is_some_and(|manifest| manifest.file_count == 1)
        && source_and_tests.predictions_written == 2
        && source_and_tests.manifest.file_count == 3
        && source_and_tests.manifest.test_file_count == 2
        && source_after.prediction_id_hex == source_prediction_id
        && after.project_prediction_rows == 3;
    Ok(json!({
        "case": "scope_switch_source_only_to_source_and_tests",
        "before": before,
        "trigger": "Incremental ingest with SourceAndTests after SourceOnly manifest",
        "after": after,
        "sourceOnlySummary": summarize_report(&source_only),
        "sourceAndTestsSummary": summarize_report(&source_and_tests),
        "sourcePredictionIdBefore": source_prediction_id,
        "sourcePredictionIdAfter": source_after.prediction_id_hex,
        "pass": pass
    }))
}

fn plain_directory_and_single_file_case(root: &Path) -> Result<Value> {
    let dir_project_id = "task-py-g-063-plain-dir";
    let file_project_id = "task-py-g-063-single-file";
    reset_project(dir_project_id)?;
    reset_project(file_project_id)?;
    let project_dir = root.join("plain_directory_project");
    init_plain_project_dir(&project_dir)?;
    fs::create_dir_all(project_dir.join("pkg/sub"))?;
    fs::write(
        project_dir.join("pkg/main.py"),
        "def main():\n    return 1\n",
    )?;
    fs::write(
        project_dir.join("pkg/sub/nested.py"),
        "def nested():\n    return 2\n",
    )?;
    fs::write(project_dir.join("notes.md"), "# plain directory project\n")?;
    fs::write(project_dir.join("pkg/sub/ignore.bin"), [0, 1, 2, 3])?;
    let dir_report = run_project_ingest(ProjectIngestRequest {
        repo_path: project_dir.clone(),
        project_id: Some(dir_project_id.to_string()),
        mode: ProjectIngestMode::Full,
        scope: ProjectIngestScope::SourceOnly,
        overwrite: false,
    })?;
    let single_file = project_dir.join("pkg/sub/nested.py");
    let file_report = run_project_ingest(ProjectIngestRequest {
        repo_path: single_file.clone(),
        project_id: Some(file_project_id.to_string()),
        mode: ProjectIngestMode::Full,
        scope: ProjectIngestScope::SourceOnly,
        overwrite: false,
    })?;
    let dir_after = inspect_state(dir_project_id)?;
    let file_after = inspect_state(file_project_id)?;
    let pass = dir_report.manifest.file_count == 2
        && dir_after.project_prediction_rows == 2
        && file_report.manifest.file_count == 1
        && file_report.manifest.predictions[0].file_path == "nested.py"
        && file_after.project_prediction_rows == 1;
    Ok(json!({
        "case": "plain_directory_recursive_and_single_file_source_paths",
        "before": {"gitRequired": false},
        "trigger": "Full SourceOnly ingest on a non-git directory tree, then direct .py file path",
        "after": {
            "directoryState": dir_after,
            "singleFileState": file_after
        },
        "directorySummary": summarize_report(&dir_report),
        "singleFileSummary": summarize_report(&file_report),
        "singleFileInput": single_file,
        "pass": pass
    }))
}

fn prediction_by_file<'a>(
    manifest: &ProjectIngestManifest,
    predictions: &'a [RealityPrediction],
    file_path: &str,
) -> Result<&'a RealityPrediction> {
    let row = manifest
        .predictions
        .iter()
        .find(|row| row.file_path == file_path)
        .with_context(|| format!("manifest row missing for {file_path}"))?;
    predictions
        .iter()
        .find(|prediction| hex::encode(prediction.prediction_id) == row.prediction_id_hex)
        .with_context(|| format!("prediction row missing for {file_path}"))
}

fn read_project_predictions(db_path: &Path, project_id: &str) -> Result<Vec<RealityPrediction>> {
    let db = open_infer_rocksdb(db_path)?;
    let cf = db
        .cf_handle(CF_MEJEPA_LIVE_PREDICTIONS)
        .context("missing live prediction CF")?;
    let prefix = format!("project_ingest:{project_id}:");
    let mut out = Vec::new();
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let (_key, value) = item?;
        let prediction: RealityPrediction = bincode::deserialize(&value)?;
        if prediction.task_id.0.starts_with(&prefix) {
            out.push(prediction);
        }
    }
    Ok(out)
}

fn verify_manifest_physical_keys(
    db_path: &Path,
    project_id: &str,
    manifest: &ProjectIngestManifest,
) -> Result<Value> {
    let db = open_infer_rocksdb(db_path)?;
    let cf = db
        .cf_handle(CF_MEJEPA_LIVE_PREDICTIONS)
        .context("missing live prediction CF")?;
    let mut keys = BTreeMap::new();
    for row in &manifest.predictions {
        let key = project_prediction_live_key(project_id, &row.file_path, &row.file_blake3);
        ensure!(key.len() == 40, "project prediction key must be 40 bytes");
        let key_hex = hex::encode(&key);
        ensure!(
            row.live_prediction_key_hex == key_hex,
            "manifest key mismatch for {}",
            row.file_path
        );
        let bytes = db
            .get_cf(cf, &key)?
            .with_context(|| format!("missing physical CF row for {}", row.file_path))?;
        let prediction: RealityPrediction = bincode::deserialize(&bytes)?;
        ensure!(
            hex::encode(prediction.prediction_id) == row.prediction_id_hex,
            "prediction id mismatch for {}",
            row.file_path
        );
        ensure!(
            prediction.task_id.0 == row.task_id,
            "task id mismatch for {}",
            row.file_path
        );
        keys.insert(
            row.file_path.clone(),
            json!({
                "keyHex": key_hex,
                "valueBytes": bytes.len(),
                "predictionId": row.prediction_id_hex,
                "taskId": row.task_id
            }),
        );
    }
    Ok(json!({
        "rowCount": keys.len(),
        "keysByFile": keys
    }))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct StateSnapshot {
    manifest_exists: bool,
    manifest: Option<ManifestSummary>,
    sqlite: SqliteCounts,
    project_prediction_rows: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ManifestSummary {
    last_ingest_unix_ms: i64,
    file_count: usize,
    prediction_count: usize,
    source_file_count: usize,
    test_file_count: usize,
    scope: ProjectIngestScope,
    prediction_ids_by_file: BTreeMap<String, String>,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct SqliteCounts {
    exists: bool,
    files: usize,
    chunks: usize,
    chunk_paths: usize,
    embeddings: usize,
}

fn inspect_state(project_id: &str) -> Result<StateSnapshot> {
    let project_root = project_root(project_id);
    let manifest_path = project_root.join("manifest.json");
    let manifest = if manifest_path.exists() {
        let manifest: ProjectIngestManifest = serde_json::from_slice(&fs::read(&manifest_path)?)?;
        Some(ManifestSummary {
            last_ingest_unix_ms: manifest.last_ingest_unix_ms,
            file_count: manifest.file_count,
            prediction_count: manifest.prediction_count,
            source_file_count: manifest.source_file_count,
            test_file_count: manifest.test_file_count,
            scope: manifest.scope,
            prediction_ids_by_file: manifest
                .predictions
                .into_iter()
                .map(|row| (row.file_path, row.prediction_id_hex))
                .collect(),
        })
    } else {
        None
    };
    let db_path = default_db_path(project_id)?;
    let rows = if db_path.exists() {
        let db = open_infer_rocksdb(&db_path)?;
        count_project_predictions(db.as_ref(), project_id)?
    } else {
        0
    };
    Ok(StateSnapshot {
        manifest_exists: manifest_path.exists(),
        manifest,
        sqlite: sqlite_counts(&project_root.join("cache/cache.db"))?,
        project_prediction_rows: rows,
    })
}

fn sqlite_counts(db_path: &Path) -> Result<SqliteCounts> {
    if !db_path.exists() {
        return Ok(SqliteCounts::default());
    }
    let conn = Connection::open(db_path)?;
    Ok(SqliteCounts {
        exists: true,
        files: count(&conn, "files")?,
        chunks: count(&conn, "chunks")?,
        chunk_paths: count(&conn, "chunk_paths")?,
        embeddings: count(&conn, "embeddings")?,
    })
}

fn chunk_updated_at_digest(db_path: &Path) -> Result<String> {
    let conn = Connection::open(db_path)?;
    let mut stmt = conn.prepare("SELECT chunk_key, updated_at FROM chunks ORDER BY chunk_key")?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
    })?;
    let mut lines = Vec::new();
    for row in rows {
        let (chunk_key, updated_at) = row?;
        lines.push(format!("{chunk_key}:{updated_at}"));
    }
    use sha2::{Digest, Sha256};
    Ok(format!("{:x}", Sha256::digest(lines.join("\n").as_bytes())))
}

fn count(conn: &Connection, table: &str) -> Result<usize> {
    let sql = format!("SELECT COUNT(*) FROM {table}");
    Ok(conn.query_row(&sql, [], |row| row.get::<_, i64>(0))? as usize)
}

fn summarize_report(report: &context_graph_mejepa::ProjectIngestReport) -> Value {
    json!({
        "projectId": report.project_id,
        "mode": report.mode,
        "scope": report.scope,
        "filesBefore": report.files_before,
        "filesAfter": report.files_after,
        "predictionsWritten": report.predictions_written,
        "newEmbeddingsWritten": report.new_embeddings_written,
        "projectPredictionRowsBefore": report.project_prediction_rows_before,
        "projectPredictionRowsAfter": report.project_prediction_rows_after,
        "manifestPath": report.manifest_path,
        "cacheDbPath": report.cache_db_path,
        "predictionsDbPath": report.predictions_db_path,
        "changedFiles": report.cache_report.changed_files,
        "deletedFiles": report.cache_report.deleted_files
    })
}

fn call_project_ingest_via_mcp(
    root: &Path,
    tool_name: &str,
    repo_path: &Path,
    project_id: &str,
) -> Result<Value> {
    let runtime = tokio::runtime::Runtime::new()?;
    let handlers = build_protocol_handlers(root)?;
    let request = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(JsonRpcId::Number(282)),
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": tool_name,
            "arguments": {
                "repoPath": repo_path,
                "projectId": project_id,
                "mode": "Full",
                "scope": "SourceOnly"
            }
        })),
    };
    let response = runtime.block_on(handlers.dispatch(request));
    ensure!(
        response.error.is_none(),
        "tools/call returned JSON-RPC error: {:?}",
        response.error
    );
    let result = response.result.context("tools/call missing result")?;
    ensure!(
        result.get("isError").and_then(Value::as_bool) == Some(false),
        "tools/call returned MCP isError: {result}"
    );
    result
        .get("structuredContent")
        .cloned()
        .context("tools/call missing structuredContent")
}

fn build_protocol_handlers(root: &Path) -> Result<Handlers> {
    let db_path = root.join(format!(
        "protocol-handler-{}",
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
    ));
    let rocksdb_store = RocksDbTeleologicalStore::open(&db_path)?;
    let db_arc = rocksdb_store.db_arc();
    let teleological_store: Arc<dyn TeleologicalMemoryStore> = Arc::new(rocksdb_store);
    let provider: Arc<dyn MultiArrayEmbeddingProvider> = Arc::new(FailClosedEmbeddingProvider);
    let edge_repository = EdgeRepository::new(db_arc);
    let graph_builder = Arc::new(BackgroundGraphBuilder::new(
        edge_repository.clone(),
        Arc::clone(&teleological_store),
        Arc::clone(&provider),
        GraphBuilderConfig::default(),
    ));
    Handlers::without_llm(
        teleological_store,
        provider,
        Arc::new(StubLayerStatusProvider),
        edge_repository,
        graph_builder,
        Arc::new(context_graph_embeddings::provider::NoOpCausalHintProvider),
    )
    .context("build protocol handlers")
}

struct FailClosedEmbeddingProvider;

#[async_trait]
impl MultiArrayEmbeddingProvider for FailClosedEmbeddingProvider {
    async fn embed_all(&self, _content: &str) -> CoreResult<MultiArrayEmbeddingOutput> {
        Err(CoreError::Internal(
            "project-ingest MCP FSV must not cross into graph embedding paths".to_string(),
        ))
    }

    async fn embed_batch_all(
        &self,
        _contents: &[String],
        _metadata: &[EmbeddingMetadata],
    ) -> CoreResult<Vec<MultiArrayEmbeddingOutput>> {
        Err(CoreError::Internal(
            "project-ingest MCP FSV must not cross into graph embedding paths".to_string(),
        ))
    }

    fn model_ids(&self) -> [&str; NUM_EMBEDDERS] {
        ["project-ingest-fsv-fail-closed"; NUM_EMBEDDERS]
    }

    fn is_ready(&self) -> bool {
        false
    }

    fn health_status(&self) -> [bool; NUM_EMBEDDERS] {
        [false; NUM_EMBEDDERS]
    }
}

fn init_plain_project_dir(repo: &Path) -> Result<()> {
    fs::create_dir_all(repo)?;
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

fn project_root(project_id: &str) -> PathBuf {
    PathBuf::from("/mnt/d/contextgraph/projects").join(project_id)
}

fn default_db_path(project_id: &str) -> Result<PathBuf> {
    Ok(project_root(project_id).join("predictions/live-predictions.rocksdb"))
}

fn reset_project(project_id: &str) -> Result<()> {
    remove_if_exists(&project_root(project_id))
}

fn remove_if_exists(path: &Path) -> Result<()> {
    if path.exists() {
        fs::remove_dir_all(path)?;
    }
    Ok(())
}

fn write_json_0600(path: &Path, value: &Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let bytes = serde_json::to_vec_pretty(value)?;
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .mode(0o600)
            .open(path)?;
        file.write_all(&bytes)?;
    }
    #[cfg(not(unix))]
    fs::write(path, &bytes)?;
    let readback = fs::read(path)?;
    ensure!(readback == bytes, "report readback bytes mismatch");
    Ok(())
}

fn case_pass(value: &Value) -> bool {
    value.get("pass").and_then(Value::as_bool).unwrap_or(false)
}
