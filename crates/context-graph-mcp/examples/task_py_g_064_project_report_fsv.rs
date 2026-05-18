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
    blake3_hex, count_project_predictions, count_project_reports, open_infer_rocksdb,
    project_prediction_created_at_unix_ms, project_prediction_id, project_prediction_live_key,
    project_prediction_session_id, read_project_report_cf, run_project_ingest, run_project_report,
    ChunkId, ChunkRole, ConformalSet, EdgeCaseClass, EmbedderId, FailureModeClass, Language,
    OracleOutcome, PredictedEdgeCase, PredictedFailureMode, PredictedSecurityConcern,
    PredictedWorks, PredictionProvenance, ProjectIngestManifest, ProjectIngestMode,
    ProjectIngestRequest, ProjectIngestScope, ProjectPredictionManifestRow, ProjectRealityReport,
    ProjectReportRequest, ProjectReportSection, RealityPrediction, RealityPredictionBuilder,
    RootCauseClass, SecurityConcernClass, Severity, TaskId, UncoveredPath, Verdict, WitnessHash,
};
use context_graph_mejepa_cf::{CF_MEJEPA_LIVE_PREDICTIONS, CF_MEJEPA_PROJECT_REPORTS};
use context_graph_storage::teleological::RocksDbTeleologicalStore;
use context_graph_storage::{BackgroundGraphBuilder, EdgeRepository, GraphBuilderConfig};
use rocksdb::{WriteBatch, WriteOptions};
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

const FSV_ROOT: &str = "/mnt/d/contextgraph/fsv/task-py-g-064-reality-report-fsv";
const REPORT_NAME: &str = "project_report_fsv.json";

fn main() -> Result<()> {
    let root = PathBuf::from(FSV_ROOT).join(format!(
        "run-{}",
        chrono::Utc::now().format("%Y%m%dT%H%M%SZ")
    ));
    fs::create_dir_all(&root)?;
    let evidence = json!({
        "schemaVersion": 1,
        "taskId": "TASK-PY-G-064",
        "issue": 283,
        "sourceOfTruth": {
            "reportJson": "/mnt/d/contextgraph/projects/<project_id>/report/report.json",
            "reportMarkdown": "/mnt/d/contextgraph/projects/<project_id>/report/report.md",
            "reportCf": CF_MEJEPA_PROJECT_REPORTS,
            "predictionCf": CF_MEJEPA_LIVE_PREDICTIONS
        },
        "mcpSurface": mcp_surface_case(&root)?,
        "happyPath": happy_path_known_bug_case(&root)?,
        "edgeCases": [
            zero_predictions_case(&root)?,
            all_pass_no_q3_case(&root)?,
            top_n_caps_large_case(&root)?,
            missing_project_fail_closed_case()?,
            invalid_project_id_case()?,
            manifest_path_mismatch_case()?,
            corrupt_live_prediction_key_case()?
        ],
    });
    let all_passed = evidence["mcpSurface"]["pass"].as_bool() == Some(true)
        && evidence["happyPath"]["pass"].as_bool() == Some(true)
        && evidence["edgeCases"]
            .as_array()
            .context("edgeCases missing")?
            .iter()
            .all(|case| case["pass"].as_bool() == Some(true));
    let mut with_status = evidence;
    with_status["allPassed"] = json!(all_passed);
    let report_path = root.join(REPORT_NAME);
    context_graph_mejepa::write_json_0600(&report_path, &with_status)?;
    let bytes = fs::read(&report_path)?;
    let readback: Value = serde_json::from_slice(&bytes)?;
    ensure!(
        readback["allPassed"].as_bool() == Some(true),
        "FSV report did not persist allPassed=true"
    );
    println!("{}", serde_json::to_string_pretty(&readback)?);
    Ok(())
}

fn mcp_surface_case(root: &Path) -> Result<Value> {
    let project_id = "task-py-g-064-mcp";
    reset_project(project_id)?;
    let repo = root.join("mcp_repo");
    write_python_project(
        &repo,
        &[("mcp_case.py", "def divide(a, b):\n    return a / b\n")],
    )?;
    run_project_ingest(ProjectIngestRequest {
        repo_path: repo,
        project_id: Some(project_id.to_string()),
        mode: ProjectIngestMode::Full,
        scope: ProjectIngestScope::SourceOnly,
        overwrite: false,
        changed_paths: Vec::new(),
    })?;
    let before = inspect_state(project_id)?;
    let canonical = context_graph_mcp::tools::names::MEJEPA_PROJECT_REPORT;
    let alias = context_graph_mcp::tools::aliases::resolve_alias("mejepa_project_report");
    let definition = context_graph_mcp::tools::definitions::get_tool_definitions()
        .into_iter()
        .find(|tool| tool.name == canonical)
        .context("canonical project report tool missing")?;
    let canonical_result = call_project_report_via_mcp(root, canonical, project_id)?;
    let alias_result = call_project_report_via_mcp(root, "mejepa_project_report", project_id)?;
    let after = inspect_state(project_id)?;
    let pass = alias == canonical
        && definition
            .input_schema
            .get("properties")
            .and_then(|value| value.get("projectId"))
            .is_some()
        && definition
            .input_schema
            .get("properties")
            .is_none_or(|properties| properties.get("dbPath").is_none())
        && canonical_result["report"]["topLikelyBrokenAreas"]
            .as_array()
            .is_some_and(|rows| rows.len() == 1)
        && alias_result["reportCf"].as_str() == Some(CF_MEJEPA_PROJECT_REPORTS)
        && after.report_cf_rows >= before.report_cf_rows + 2;
    Ok(json!({
        "case": "mcp_tool_definition_and_alias",
        "before": before,
        "after": after,
        "canonical": canonical,
        "alias": alias,
        "schema": definition.input_schema,
        "canonicalReportVersion": canonical_result["reportVersion"],
        "aliasReportVersion": alias_result["reportVersion"],
        "pass": pass
    }))
}

fn happy_path_known_bug_case(root: &Path) -> Result<Value> {
    let project_id = "task-py-g-064-happy";
    reset_project(project_id)?;
    let repo = root.join("happy_repo");
    write_python_project(
        &repo,
        &[
            ("src/clean.py", "def add(a, b):\n    return a + b\n"),
            ("src/bug.py", "def divide(a, b):\n    return a / b\n"),
            ("src/security.py", "def run(expr):\n    return eval(expr)\n"),
        ],
    )?;
    let before = inspect_state(project_id)?;
    run_project_ingest(ProjectIngestRequest {
        repo_path: repo,
        project_id: Some(project_id.to_string()),
        mode: ProjectIngestMode::Full,
        scope: ProjectIngestScope::SourceOnly,
        overwrite: false,
        changed_paths: Vec::new(),
    })?;
    let run = run_project_report(report_request(project_id))?;
    let after = inspect_state(project_id)?;
    let json_readback = read_report_json(&run.report_json_path)?;
    let cf_readback = reopened_report_cf(project_id, &run.report_version)?;
    let md = fs::read_to_string(&run.report_md_path)?;
    let bug = run
        .report
        .top_likely_broken_areas
        .first()
        .context("missing broken-area row")?;
    let pass = after.report_json_exists
        && after.report_md_exists
        && after.report_cf_rows == 1
        && after.prediction_rows == 3
        && run.report.verdict_summary.total == 3
        && bug.file_path == "src/bug.py"
        && bug.explanation.contains("DivisionByZero")
        && run.report.top_edge_cases_not_exercised.len() == 1
        && run.report.top_vulnerability_slices.len() == 1
        && run.report.top_vulnerability_slices[0].file_path == "src/security.py"
        && run.report.predicted_works.len() == 1
        && json_readback.report_version == run.report_version
        && cf_readback.report_version == run.report_version
        && md.contains("Top-20 likely-broken areas");
    Ok(json!({
        "case": "happy_known_bug_report",
        "before": before,
        "after": after,
        "reportVersion": run.report_version,
        "reportPaths": {
            "json": run.report_json_path,
            "markdown": run.report_md_path,
            "db": run.report_db_path
        },
        "sourceOfTruthReadback": {
            "jsonVersion": json_readback.report_version,
            "cfVersion": cf_readback.report_version,
            "markdownBytes": md.len(),
            "bugPredictionId": bug.evidence.prediction_id,
            "bugLiveKey": bug.evidence.live_prediction_key_hex
        },
        "pass": pass
    }))
}

fn zero_predictions_case(_root: &Path) -> Result<Value> {
    let project_id = "task-py-g-064-zero";
    reset_project(project_id)?;
    let before = inspect_state(project_id)?;
    write_manual_project(project_id, Vec::new())?;
    let manifest_state = inspect_state(project_id)?;
    let run = run_project_report(report_request(project_id))?;
    let after = inspect_state(project_id)?;
    let json_readback = read_report_json(&run.report_json_path)?;
    let pass = !before.manifest_exists
        && manifest_state.manifest_exists
        && after.report_cf_rows == 1
        && run.report.warning.is_some()
        && run.report.verdict_summary.total == 0
        && run.report.top_likely_broken_areas.is_empty()
        && json_readback.warning.is_some();
    Ok(json!({
        "case": "zero_predictions_renders_warning",
        "before": before,
        "afterManifest": manifest_state,
        "after": after,
        "warning": run.report.warning,
        "pass": pass
    }))
}

fn all_pass_no_q3_case(_root: &Path) -> Result<Value> {
    let project_id = "task-py-g-064-all-pass";
    reset_project(project_id)?;
    let before = inspect_state(project_id)?;
    let specs = (0..3)
        .map(|idx| SyntheticPredictionSpec {
            file_path: format!("pkg/pass_{idx}.py"),
            kind: SyntheticPredictionKind::Pass,
        })
        .collect();
    write_manual_project(project_id, specs)?;
    let run = run_project_report(report_request(project_id))?;
    let after = inspect_state(project_id)?;
    let pass = before.prediction_rows == 0
        && after.prediction_rows == 3
        && after.report_cf_rows == 1
        && run.report.verdict_summary.pass == 3
        && run.report.top_likely_broken_areas.is_empty()
        && run.report.predicted_works.len() == 3;
    Ok(json!({
        "case": "all_pass_project_has_empty_q3_section",
        "before": before,
        "after": after,
        "predictedWorks": run.report.predicted_works.len(),
        "pass": pass
    }))
}

fn top_n_caps_large_case(_root: &Path) -> Result<Value> {
    let project_id = "task-py-g-064-large";
    reset_project(project_id)?;
    let before = inspect_state(project_id)?;
    let specs = (0..10_005)
        .map(|idx| SyntheticPredictionSpec {
            file_path: format!("pkg/m{idx:05}.py"),
            kind: if idx % 2 == 0 {
                SyntheticPredictionKind::Bug
            } else {
                SyntheticPredictionKind::Cold
            },
        })
        .collect();
    write_manual_project(project_id, specs)?;
    let run = run_project_report(report_request(project_id))?;
    let page = run_project_report(ProjectReportRequest {
        project_id: project_id.to_string(),
        section: Some(ProjectReportSection::Predictions),
        offset: Some(100),
    })?;
    let after = inspect_state(project_id)?;
    let pass = before.prediction_rows == 0
        && after.prediction_rows == 10_005
        && run.report.pagination.prediction_rows_total == 10_005
        && run.report.pagination.has_more_prediction_rows
        && run.report.pagination.all_predictions_link.is_some()
        && page.prediction_rows_page.as_ref().is_some_and(|page| {
            page.offset == 100
                && page.limit == 100
                && page.total == 10_005
                && page.rows.len() == 100
                && page.has_more
                && page.next_link.is_some()
        })
        && run.report.pagination.likely_broken_total_before_cap
            > run.report.caps.likely_broken_areas
        && run.report.pagination.cold_cells_total_before_cap > run.report.caps.cold_cells
        && run.report.top_likely_broken_areas.len() == run.report.caps.likely_broken_areas
        && run.report.top_edge_cases_not_exercised.len() == run.report.caps.edge_cases
        && run.report.top_vulnerability_slices.len() == run.report.caps.vulnerability_slices
        && run.report.coverage_prediction_map.len() == run.report.caps.coverage_map
        && run.report.cold_cells.len() == run.report.caps.cold_cells;
    Ok(json!({
        "case": "large_project_top_n_caps_and_pagination",
        "before": before,
        "after": after,
        "caps": run.report.caps,
        "pagination": run.report.pagination,
        "predictionPage": page.prediction_rows_page,
        "sectionLengths": {
            "broken": run.report.top_likely_broken_areas.len(),
            "edge": run.report.top_edge_cases_not_exercised.len(),
            "vulnerability": run.report.top_vulnerability_slices.len(),
            "coverage": run.report.coverage_prediction_map.len(),
            "cold": run.report.cold_cells.len()
        },
        "pass": pass
    }))
}

fn missing_project_fail_closed_case() -> Result<Value> {
    let project_id = "task-py-g-064-missing";
    reset_project(project_id)?;
    let before = inspect_state(project_id)?;
    let err = run_project_report(report_request(project_id))
        .expect_err("missing project should fail closed");
    let after = inspect_state(project_id)?;
    let pass = err.code() == "MEJEPA_PROJECT_REPORT_INGEST_REQUIRED"
        && !after.report_json_exists
        && !after.report_md_exists
        && before == after;
    Ok(json!({
        "case": "missing_project_fails_closed",
        "before": before,
        "after": after,
        "errorCode": err.code(),
        "pass": pass
    }))
}

fn invalid_project_id_case() -> Result<Value> {
    let err =
        run_project_report(report_request(".")).expect_err("dot project id should fail closed");
    let pass = err.code() == "MEJEPA_PROJECT_REPORT_INVALID_PROJECT_ID";
    Ok(json!({
        "case": "invalid_dot_project_id_fails_closed",
        "errorCode": err.code(),
        "pass": pass
    }))
}

fn manifest_path_mismatch_case() -> Result<Value> {
    let project_id = "task-py-g-064-manifest-tamper";
    reset_project(project_id)?;
    let mut manifest = write_manual_project(
        project_id,
        vec![SyntheticPredictionSpec {
            file_path: "pkg/pass.py".to_string(),
            kind: SyntheticPredictionKind::Pass,
        }],
    )?;
    manifest.predictions_db_path = "/tmp/contextgraph-tampered-report.rocksdb".to_string();
    context_graph_mejepa::write_json_0600(
        &PathBuf::from(format!(
            "/mnt/d/contextgraph/projects/{project_id}/manifest.json"
        )),
        &manifest,
    )?;
    let err = run_project_report(report_request(project_id))
        .expect_err("tampered manifest DB path should fail closed");
    let after = inspect_state(project_id)?;
    let pass = err.code() == "MEJEPA_PROJECT_REPORT_MANIFEST_PATH_MISMATCH"
        && !after.report_json_exists
        && !after.report_md_exists
        && after.report_cf_rows == 0;
    Ok(json!({
        "case": "manifest_prediction_db_path_mismatch_fails_closed",
        "after": after,
        "errorCode": err.code(),
        "pass": pass
    }))
}

fn corrupt_live_prediction_key_case() -> Result<Value> {
    let project_id = "task-py-g-064-corrupt-live-key";
    reset_project(project_id)?;
    let manifest = write_manual_project(
        project_id,
        vec![SyntheticPredictionSpec {
            file_path: "pkg/pass.py".to_string(),
            kind: SyntheticPredictionKind::Pass,
        }],
    )?;
    let row = manifest
        .predictions
        .first()
        .context("missing manifest row")?;
    let db = open_infer_rocksdb(&manifest.predictions_db_path)?;
    let cf = db
        .cf_handle(CF_MEJEPA_LIVE_PREDICTIONS)
        .context("live prediction CF missing")?;
    let key = project_prediction_live_key(project_id, &row.file_path, &row.file_blake3);
    let bytes = db
        .get_cf(cf, &key)?
        .context("missing live prediction row")?;
    let mut prediction: RealityPrediction = bincode::deserialize(&bytes)?;
    prediction.created_at_unix_ms += 1;
    let corrupt = bincode::serialize(&prediction)?;
    let mut opts = WriteOptions::default();
    opts.set_sync(true);
    db.put_cf_opt(cf, &key, &corrupt, &opts)?;
    db.flush_cf(cf)?;
    drop(db);
    let err = run_project_report(report_request(project_id))
        .expect_err("timestamp/key mismatch should fail closed");
    let after = inspect_state(project_id)?;
    let pass = err.code() == "MEJEPA_INFER_INVALID_INPUT"
        && !after.report_json_exists
        && !after.report_md_exists
        && after.report_cf_rows == 0;
    Ok(json!({
        "case": "corrupt_live_prediction_timestamp_fails_closed",
        "after": after,
        "errorCode": err.code(),
        "error": err.to_string(),
        "pass": pass
    }))
}

fn report_request(project_id: &str) -> ProjectReportRequest {
    ProjectReportRequest {
        project_id: project_id.to_string(),
        section: None,
        offset: None,
    }
}

fn write_manual_project(
    project_id: &str,
    specs: Vec<SyntheticPredictionSpec>,
) -> Result<ProjectIngestManifest> {
    let project_root = PathBuf::from(format!("/mnt/d/contextgraph/projects/{project_id}"));
    fs::create_dir_all(project_root.join("predictions"))?;
    fs::create_dir_all(project_root.join("report"))?;
    let predictions_db_path = project_root.join("predictions/live-predictions.rocksdb");
    let db = open_infer_rocksdb(&predictions_db_path)?;
    let cf = db
        .cf_handle(CF_MEJEPA_LIVE_PREDICTIONS)
        .context("live predictions CF missing")?;
    let mut batch = WriteBatch::default();
    let mut rows = Vec::with_capacity(specs.len());
    for (idx, spec) in specs.iter().enumerate() {
        let file_blake3 = blake3_hex(format!("{}\0{}", spec.file_path, idx).as_bytes());
        let prediction =
            synthetic_prediction(project_id, &spec.file_path, &file_blake3, spec.kind, idx)?;
        let key = project_prediction_live_key(project_id, &spec.file_path, &file_blake3);
        let value = bincode::serialize(&prediction)?;
        batch.put_cf(cf, &key, &value);
        rows.push(ProjectPredictionManifestRow {
            file_path: spec.file_path.clone(),
            file_blake3,
            role: ChunkRole::Source,
            size_bytes: 32,
            chunk_count: 1,
            prediction_id_hex: hex::encode(prediction.prediction_id),
            verdict: prediction.verdict,
            predicted_failure_modes: prediction.predicted_failure_modes.len(),
            predicted_security_concerns: prediction.predicted_security_concerns.len(),
            task_id: prediction.task_id.0,
            live_prediction_key_hex: hex::encode(key),
        });
    }
    let mut opts = WriteOptions::default();
    opts.set_sync(true);
    db.write_opt(batch, &opts)?;
    db.flush()?;
    let merkle_root = blake3_hex(format!("manifest\0{project_id}\0{}", rows.len()).as_bytes());
    let manifest = ProjectIngestManifest {
        schema_version: 1,
        project_id: project_id.to_string(),
        repo_path: format!("/mnt/d/contextgraph/projects/{project_id}/synthetic"),
        project_root: project_root.display().to_string(),
        mode: ProjectIngestMode::Full,
        scope: ProjectIngestScope::SourceOnly,
        last_ingest_unix_ms: 1_779_200_000_000 + rows.len() as i64,
        merkle_root,
        file_count: rows.len(),
        source_file_count: rows.len(),
        test_file_count: 0,
        doc_file_count: 0,
        config_file_count: 0,
        prediction_count: rows.len(),
        cache_db_path: project_root.join("cache/cache.db").display().to_string(),
        predictions_db_path: predictions_db_path.display().to_string(),
        predictions: rows,
        changed_files: Vec::new(),
        deleted_files: Vec::new(),
        new_embeddings_written: 0,
    };
    context_graph_mejepa::write_json_0600(&project_root.join("manifest.json"), &manifest)?;
    Ok(manifest)
}

fn synthetic_prediction(
    project_id: &str,
    file_path: &str,
    file_blake3: &str,
    kind: SyntheticPredictionKind,
    idx: usize,
) -> Result<RealityPrediction> {
    let chunk = ChunkId(format!(
        "project_report:{project_id}:{}",
        &blake3_hex(format!("{file_path}\0{file_blake3}").as_bytes())[..16]
    ));
    let prediction_id = project_prediction_id(project_id, file_path, file_blake3);
    let task_id = TaskId(format!(
        "project_ingest:{project_id}:{}",
        &blake3_hex(format!("{file_path}\0{file_blake3}").as_bytes())[..32]
    ));
    let mut builder = RealityPredictionBuilder::from_parts(
        task_id,
        project_prediction_session_id(project_id),
        Language::Python,
        ConformalSet::try_new(vec![OracleOutcome::Abstain], 0.10, 0.50)?,
    )
    .prediction_id(prediction_id)
    .witness_hash(WitnessHash(id32(&format!(
        "witness\0{project_id}\0{file_path}"
    ))))
    .covered_chunks(vec![chunk.clone()])
    .predicted_test_pass(vec![0.5])
    .ood_score(0.15)
    .calibrated_confidence(0.82)
    .provenance(PredictionProvenance {
        predictor_version: "task-py-g-063-project-ingest-v1".to_string(),
        constellation_version: "task-py-g-064-fsv".to_string(),
        calibration_version: "synthetic-known-expected".to_string(),
        active_pointer: format!("project={project_id};file={file_path}"),
    })
    .source_panel_sha(id32(&format!("panel\0{project_id}\0{file_path}")))
    .calibration_version("synthetic-known-expected")
    .created_at_unix_ms(project_prediction_created_at_unix_ms(
        project_id,
        file_path,
        file_blake3,
    ));

    match kind {
        SyntheticPredictionKind::Pass => {
            builder = builder
                .verdict(Verdict::Pass)
                .predicted_oracle_pass(0.91)
                .predicted_works(vec![PredictedWorks {
                    chunk,
                    line_range: (1, 2),
                    claim: format!("{file_path} has high-confidence positive behavior"),
                    confidence: 0.91,
                    supporting_embedders: vec![EmbedderId("E_AST".to_string())],
                    similar_known_good_exemplars: Vec::new(),
                    evidence_strength: 0.88,
                }]);
        }
        SyntheticPredictionKind::Bug => {
            builder = builder
                .verdict(Verdict::Fail)
                .predicted_oracle_pass(0.12)
                .predicted_failure_modes(vec![PredictedFailureMode {
                    failure_class: FailureModeClass::Exception,
                    chunk: chunk.clone(),
                    line_range: (2, 2),
                    confidence: 0.99 - ((idx % 50) as f32 * 0.001),
                    severity: Severity::High,
                    explanation: "synthetic division-by-zero bug used for report ranking"
                        .to_string(),
                    contributing_embedders: vec![EmbedderId("E_AST".to_string())],
                    root_cause_class: RootCauseClass::LogicError,
                }])
                .predicted_edge_cases(vec![PredictedEdgeCase {
                    edge_class: EdgeCaseClass::BoundaryValue,
                    chunk: chunk.clone(),
                    line_range: (2, 2),
                    triggering_input_description: "denominator equals zero".to_string(),
                    covered_by_test: false,
                    confidence: 0.95,
                }])
                .predicted_uncovered_paths(vec![UncoveredPath {
                    chunk: chunk.clone(),
                    line_range: (2, 2),
                    path_description: "zero denominator path is not covered by tests".to_string(),
                    defect_probability: 0.93,
                    confidence: 0.94,
                    evidence: "synthetic uncovered-path signal".to_string(),
                }])
                .predicted_security_concerns(vec![PredictedSecurityConcern {
                    class: SecurityConcernClass::Other,
                    chunk,
                    line_range: (3, 3),
                    cvss_estimate: Some(7.0 + (idx % 30) as f32 / 10.0),
                    explanation: "synthetic vulnerability slice used for top-N caps".to_string(),
                }]);
        }
        SyntheticPredictionKind::Cold => {
            builder = builder
                .verdict(Verdict::OutOfDistribution)
                .predicted_oracle_pass(0.41)
                .ood_score(0.91)
                .calibrated_confidence(0.20);
        }
    }
    Ok(builder.build()?)
}

fn id32(input: &str) -> [u8; 32] {
    let digest = Sha256::digest(input.as_bytes());
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest[..32]);
    out
}

#[derive(Debug, Clone)]
struct SyntheticPredictionSpec {
    file_path: String,
    kind: SyntheticPredictionKind,
}

#[derive(Debug, Clone, Copy)]
enum SyntheticPredictionKind {
    Pass,
    Bug,
    Cold,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectReportState {
    manifest_exists: bool,
    report_json_exists: bool,
    report_md_exists: bool,
    report_cf_rows: usize,
    prediction_rows: usize,
    report_json_bytes: u64,
    report_md_bytes: u64,
}

fn inspect_state(project_id: &str) -> Result<ProjectReportState> {
    let root = PathBuf::from(format!("/mnt/d/contextgraph/projects/{project_id}"));
    let manifest = root.join("manifest.json");
    let report_json = root.join("report/report.json");
    let report_md = root.join("report/report.md");
    let db_path = root.join("predictions/live-predictions.rocksdb");
    let (report_cf_rows, prediction_rows) = if db_path.exists() {
        let db = open_infer_rocksdb(&db_path)?;
        (
            count_project_reports(db.as_ref(), project_id)?,
            count_project_predictions(db.as_ref(), project_id)?,
        )
    } else {
        (0, 0)
    };
    Ok(ProjectReportState {
        manifest_exists: manifest.exists(),
        report_json_exists: report_json.exists(),
        report_md_exists: report_md.exists(),
        report_cf_rows,
        prediction_rows,
        report_json_bytes: file_len(&report_json),
        report_md_bytes: file_len(&report_md),
    })
}

fn file_len(path: &Path) -> u64 {
    fs::metadata(path).map(|meta| meta.len()).unwrap_or(0)
}

fn read_report_json(path: &str) -> Result<ProjectRealityReport> {
    let bytes = fs::read(path)?;
    Ok(serde_json::from_slice(&bytes)?)
}

fn reopened_report_cf(project_id: &str, report_version: &str) -> Result<ProjectRealityReport> {
    let db = open_infer_rocksdb(format!(
        "/mnt/d/contextgraph/projects/{project_id}/predictions/live-predictions.rocksdb"
    ))?;
    Ok(read_project_report_cf(
        db.as_ref(),
        project_id,
        report_version,
    )?)
}

fn write_python_project(root: &Path, files: &[(&str, &str)]) -> Result<()> {
    for (rel, text) in files {
        let path = root.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, text)?;
    }
    Ok(())
}

fn reset_project(project_id: &str) -> Result<()> {
    let root = PathBuf::from(format!("/mnt/d/contextgraph/projects/{project_id}"));
    if root.exists() {
        fs::remove_dir_all(&root)?;
    }
    Ok(())
}

fn call_project_report_via_mcp(root: &Path, tool_name: &str, project_id: &str) -> Result<Value> {
    let runtime = tokio::runtime::Runtime::new()?;
    let handlers = build_protocol_handlers(root)?;
    let request = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(JsonRpcId::Number(283)),
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": tool_name,
            "arguments": {
                "projectId": project_id
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
            "project-report MCP FSV must not cross into graph embedding paths".to_string(),
        ))
    }

    async fn embed_batch_all(
        &self,
        _contents: &[String],
        _metadata: &[EmbeddingMetadata],
    ) -> CoreResult<Vec<MultiArrayEmbeddingOutput>> {
        Err(CoreError::Internal(
            "project-report MCP FSV must not cross into graph embedding paths".to_string(),
        ))
    }

    fn model_ids(&self) -> [&str; NUM_EMBEDDERS] {
        ["project-report-fsv-fail-closed"; NUM_EMBEDDERS]
    }

    fn is_ready(&self) -> bool {
        false
    }

    fn health_status(&self) -> [bool; NUM_EMBEDDERS] {
        [false; NUM_EMBEDDERS]
    }
}
