use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{ensure, Context, Result};
use context_graph_mejepa::head_projection::{
    count_head_projection_registry, head_projection_registry_records, project_head,
    project_per_head_dda_rows, read_head_projection_registry,
    write_head_projection_registry_sync_readback, E_AST, E_CFG, E_DATA_FLOW, E_DIFF, E_ORACLE,
    E_REASONING, E_STATIC_ANALYSIS, E_TEST, E_TYPE_GRAPH, HEAD_PROJECTION_SCHEMA,
    HEAD_PROJECTION_SCHEMA_VERSION,
};
use context_graph_mejepa::{
    open_infer_rocksdb, write_json_0600, ChunkId, DdaSignals, EmbedderId, HeadId,
    HeadProjectionInput, PanelId,
};
use context_graph_mejepa_cf::{CF_MEJEPA_DDA_SIGNALS, CF_MEJEPA_HEAD_PROJECTION_REGISTRY};
use context_graph_mejepa_train::dda::{
    count_persisted_dda_signals, persist_dda_signals, read_dda_signals,
};
use serde::Serialize;
use sha2::{Digest, Sha256};

const FSV_ROOT: &str = "/mnt/d/contextgraph/fsv/task-py-g-043-per-head-dda-projection-fsv";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct HeadFeatureReport {
    head: HeadId,
    chunk_count: usize,
    feature_count_per_chunk: usize,
    first_chunk_feature_names: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RegistryReport {
    cf: String,
    rows_before: usize,
    rows_written: usize,
    rows_after: usize,
    rows_after_reopen: usize,
    byte_identical_readback: bool,
    schema_hashes_distinct: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct EdgeCaseReport {
    missing_reasoning_code: String,
    schema_mismatch_code: String,
    empty_dda_code: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TaskPyG043FsvReport {
    task_id: String,
    issue: String,
    status: String,
    schema: String,
    schema_version: u32,
    run_root: PathBuf,
    db_path: PathBuf,
    input_cf: String,
    registry: RegistryReport,
    panel_id_hex: String,
    representative_python_chunks: usize,
    dda_rows_before: usize,
    dda_rows_written: usize,
    dda_rows_read_back: usize,
    head_count: usize,
    oracle_feature_count_per_chunk: usize,
    failure_mode_feature_count_per_chunk: usize,
    reasoning_feature_count_per_chunk: usize,
    independent_head_feature_sets_differ: bool,
    head_reports: Vec<HeadFeatureReport>,
    edge_cases: EdgeCaseReport,
    all_passed: bool,
}

fn main() -> Result<()> {
    let run_id = chrono::Utc::now().timestamp_millis();
    let run_root = PathBuf::from(FSV_ROOT).join(format!("run-{run_id}"));
    let db_path = run_root.join("rocksdb");
    fs::create_dir_all(&run_root).context("create TASK-PY-G-043 FSV run root")?;

    let panel_id = PanelId([0x43; 32]);
    let embedder_order = canonical_embedder_order();
    let rows = representative_python_dda_rows(embedder_order.len());
    let scalars_by_chunk = scalar_inputs(&rows);

    let (dda_rows_before, dda_rows_written, dda_rows_read_back, projections, registry) = {
        let db = open_infer_rocksdb(&db_path).context("open FSV RocksDB")?;
        let dda_rows_before =
            count_persisted_dda_signals(db.as_ref()).context("count DDA rows before")?;
        for (chunk_id, signals) in &rows {
            persist_dda_signals(db.as_ref(), &panel_id, chunk_id, signals)
                .with_context(|| format!("persist DDA row {}", chunk_id.0))?;
        }
        let dda_rows_written =
            count_persisted_dda_signals(db.as_ref()).context("count DDA rows after write")?;
        let mut rows_from_cf = Vec::new();
        for (chunk_id, _) in &rows {
            let readback = read_dda_signals(db.as_ref(), &panel_id, chunk_id)
                .with_context(|| format!("read DDA row {}", chunk_id.0))?
                .context("DDA row missing immediately after write")?;
            rows_from_cf.push((chunk_id.clone(), readback));
        }
        let projections =
            project_per_head_dda_rows(panel_id, &rows_from_cf, &embedder_order, &scalars_by_chunk)
                .context("project per-head DDA rows from CF readback")?;

        let registry_before =
            count_head_projection_registry(db.as_ref()).context("count registry rows before")?;
        let records = head_projection_registry_records(run_id).context("build registry records")?;
        let write_summary = write_head_projection_registry_sync_readback(db.as_ref(), &records)
            .context("write head projection registry")?;
        let registry_after =
            count_head_projection_registry(db.as_ref()).context("count registry rows after")?;
        let readback_registry =
            read_head_projection_registry(db.as_ref()).context("read registry rows")?;
        ensure!(
            readback_registry == records,
            "registry readback rows differ from encoded records"
        );
        drop(db);

        let reopened = open_infer_rocksdb(&db_path).context("reopen FSV RocksDB")?;
        let reopened_registry =
            read_head_projection_registry(reopened.as_ref()).context("read reopened registry")?;
        ensure!(
            reopened_registry == records,
            "reopened registry rows differ from encoded records"
        );
        let schema_hashes_distinct = reopened_registry
            .iter()
            .map(|row| row.feature_schema_hash.clone())
            .collect::<BTreeSet<_>>()
            .len();
        let registry = RegistryReport {
            cf: CF_MEJEPA_HEAD_PROJECTION_REGISTRY.to_string(),
            rows_before: registry_before,
            rows_written: write_summary.rows_written,
            rows_after: registry_after,
            rows_after_reopen: reopened_registry.len(),
            byte_identical_readback: write_summary.byte_identical_readback,
            schema_hashes_distinct,
        };
        (
            dda_rows_before,
            dda_rows_written,
            rows_from_cf.len(),
            projections,
            registry,
        )
    };

    let head_reports = summarize_heads(&projections)?;
    let oracle = projection_count(&head_reports, HeadId::Oracle)?;
    let failure_mode = projection_count(&head_reports, HeadId::FailureMode)?;
    let reasoning = projection_count(&head_reports, HeadId::Reasoning)?;
    let oracle_names = feature_names(&head_reports, HeadId::Oracle)?;
    let failure_names = feature_names(&head_reports, HeadId::FailureMode)?;
    let independent_head_feature_sets_differ = oracle_names != failure_names;
    let edge_cases = run_edge_cases(panel_id).context("run fail-closed edge cases")?;

    ensure!(
        dda_rows_before == 0,
        "fresh FSV DB unexpectedly had DDA rows"
    );
    ensure!(dda_rows_written == 30, "expected 30 DDA rows written");
    ensure!(dda_rows_read_back == 30, "expected 30 DDA rows read back");
    ensure!(
        projections.len() == HeadId::ALL.len(),
        "projected head count mismatch"
    );
    ensure!(oracle == 3, "oracle head must see 3 DDA scalars per chunk");
    ensure!(
        failure_mode == 5,
        "failure-mode head must see 5 DDA scalars per chunk"
    );
    ensure!(reasoning == 2, "reasoning head must see 2 pairwise scalars");
    ensure!(
        independent_head_feature_sets_differ,
        "oracle and failure-mode feature slices must differ"
    );
    ensure!(
        registry.rows_before == 0
            && registry.rows_written == HeadId::ALL.len()
            && registry.rows_after == HeadId::ALL.len()
            && registry.rows_after_reopen == HeadId::ALL.len()
            && registry.byte_identical_readback
            && registry.schema_hashes_distinct >= 9,
        "registry readback invariants failed"
    );
    ensure!(
        edge_cases.missing_reasoning_code == "HEAD_PROJECTION_MISSING_SLICE",
        "missing E_Reasoning did not return HEAD_PROJECTION_MISSING_SLICE"
    );
    ensure!(
        edge_cases.schema_mismatch_code == "HEAD_PROJECTION_SCHEMA_MISMATCH",
        "schema mismatch did not return HEAD_PROJECTION_SCHEMA_MISMATCH"
    );
    ensure!(
        edge_cases.empty_dda_code == "HEAD_PROJECTION_NO_DDA",
        "empty DDA did not return HEAD_PROJECTION_NO_DDA"
    );

    let report = TaskPyG043FsvReport {
        task_id: "TASK-PY-G-043".to_string(),
        issue: "https://github.com/ChrisRoyse/contextgraph/issues/262".to_string(),
        status: "pass".to_string(),
        schema: HEAD_PROJECTION_SCHEMA.to_string(),
        schema_version: HEAD_PROJECTION_SCHEMA_VERSION,
        run_root: run_root.clone(),
        db_path: db_path.clone(),
        input_cf: CF_MEJEPA_DDA_SIGNALS.to_string(),
        registry,
        panel_id_hex: hex::encode(panel_id.0),
        representative_python_chunks: 30,
        dda_rows_before,
        dda_rows_written,
        dda_rows_read_back,
        head_count: projections.len(),
        oracle_feature_count_per_chunk: oracle,
        failure_mode_feature_count_per_chunk: failure_mode,
        reasoning_feature_count_per_chunk: reasoning,
        independent_head_feature_sets_differ,
        head_reports,
        edge_cases,
        all_passed: true,
    };

    let report_path = run_root.join("head_projection_fsv.json");
    write_json_0600(&report_path, &report).context("write FSV report")?;
    let report_sha256 = sha256_file(&report_path).context("hash FSV report")?;
    let canonical_path = PathBuf::from(FSV_ROOT).join("head_projection_fsv.latest.json");
    write_json_0600(&canonical_path, &report).context("write latest FSV report")?;

    println!("TASK-PY-G-043 FSV PASS");
    println!("report_path={}", report_path.display());
    println!("report_sha256={report_sha256}");
    println!("oracle_feature_count_per_chunk={oracle}");
    println!("failure_mode_feature_count_per_chunk={failure_mode}");
    println!(
        "registry_rows_after_reopen={}",
        report.registry.rows_after_reopen
    );
    Ok(())
}

fn canonical_embedder_order() -> Vec<EmbedderId> {
    [
        E_ORACLE,
        E_TEST,
        E_REASONING,
        E_AST,
        E_CFG,
        E_DATA_FLOW,
        E_TYPE_GRAPH,
        E_STATIC_ANALYSIS,
        E_DIFF,
        "E_Runtime",
        "E_Witness",
        "E_LiveSession",
    ]
    .into_iter()
    .map(|value| EmbedderId(value.to_string()))
    .collect()
}

fn representative_python_dda_rows(embedder_count: usize) -> Vec<(ChunkId, DdaSignals)> {
    (0..30usize)
        .map(|idx| {
            (
                ChunkId(format!("python-panel-chunk-{idx:03}")),
                build_signals(embedder_count, idx),
            )
        })
        .collect()
}

fn build_signals(embedder_count: usize, seed: usize) -> DdaSignals {
    let pair_count = embedder_count * (embedder_count - 1) / 2;
    DdaSignals {
        per_embedder_cosine: (0..embedder_count)
            .map(|idx| unitish(seed + idx, 1.6, -0.8))
            .collect(),
        pairwise_cosine_upper: (0..pair_count)
            .map(|idx| unitish(seed * 3 + idx, 1.4, -0.7))
            .collect(),
        pairwise_mi_upper: (0..pair_count)
            .map(|idx| 0.01 + ((seed + idx) % 17) as f32 / 100.0)
            .collect(),
        blind_spot_z_scores: (0..pair_count)
            .map(|idx| ((seed + idx) % 11) as f32 / 10.0)
            .collect(),
    }
}

fn unitish(seed: usize, width: f32, offset: f32) -> f32 {
    ((seed % 101) as f32 / 100.0) * width + offset
}

fn scalar_inputs(rows: &[(ChunkId, DdaSignals)]) -> BTreeMap<ChunkId, BTreeMap<String, f32>> {
    rows.iter()
        .enumerate()
        .map(|(idx, (chunk_id, _))| {
            let mut scalars = BTreeMap::new();
            scalars.insert("coverage_delta".to_string(), idx as f32 / 1000.0);
            scalars.insert("complexity_delta".to_string(), 1.0 + idx as f32);
            scalars.insert("length_delta".to_string(), 3.0 + idx as f32);
            scalars.insert("nesting_delta".to_string(), (idx % 4) as f32);
            scalars.insert("runtime_latency_delta".to_string(), 10.0 + idx as f32);
            scalars.insert("runtime_vram_delta".to_string(), 512.0 + idx as f32);
            scalars.insert("runtime_cost_delta".to_string(), idx as f32 / 10.0);
            scalars.insert("sast_findings".to_string(), (idx % 3) as f32);
            scalars.insert("metric_delta".to_string(), 0.5 - idx as f32 / 1000.0);
            scalars.insert("cost_ci_minutes_delta".to_string(), idx as f32 / 20.0);
            scalars.insert("cost_tokens_delta".to_string(), 100.0 + idx as f32);
            (chunk_id.clone(), scalars)
        })
        .collect()
}

fn summarize_heads(
    projections: &BTreeMap<HeadId, Vec<context_graph_mejepa::HeadProjection>>,
) -> Result<Vec<HeadFeatureReport>> {
    let mut reports = Vec::new();
    for head in HeadId::ALL {
        let rows = projections
            .get(&head)
            .with_context(|| format!("missing projection rows for {}", head.as_str()))?;
        ensure!(rows.len() == 30, "{} row count mismatch", head.as_str());
        let first = rows
            .first()
            .with_context(|| format!("missing first row for {}", head.as_str()))?;
        for row in rows {
            ensure!(
                row.features.len() == first.features.len(),
                "{} feature width changed across chunks",
                head.as_str()
            );
        }
        reports.push(HeadFeatureReport {
            head,
            chunk_count: rows.len(),
            feature_count_per_chunk: first.features.len(),
            first_chunk_feature_names: first
                .features
                .iter()
                .map(|feature| feature.name.clone())
                .collect(),
        });
    }
    Ok(reports)
}

fn projection_count(reports: &[HeadFeatureReport], head: HeadId) -> Result<usize> {
    Ok(reports
        .iter()
        .find(|report| report.head == head)
        .with_context(|| format!("missing report for {}", head.as_str()))?
        .feature_count_per_chunk)
}

fn feature_names(reports: &[HeadFeatureReport], head: HeadId) -> Result<Vec<String>> {
    Ok(reports
        .iter()
        .find(|report| report.head == head)
        .with_context(|| format!("missing report for {}", head.as_str()))?
        .first_chunk_feature_names
        .clone())
}

fn run_edge_cases(panel_id: PanelId) -> Result<EdgeCaseReport> {
    let chunk_id = ChunkId("edge-case-chunk".to_string());
    let missing_reasoning_order = vec![
        EmbedderId(E_ORACLE.to_string()),
        EmbedderId(E_TEST.to_string()),
        EmbedderId(E_AST.to_string()),
    ];
    let missing_reasoning_signals = build_signals(missing_reasoning_order.len(), 77);
    let missing_reasoning_code = project_head(
        HeadId::Oracle,
        HeadProjectionInput {
            panel_id,
            chunk_id: &chunk_id,
            signals: &missing_reasoning_signals,
            embedder_order: &missing_reasoning_order,
            scalar_features: &BTreeMap::new(),
            schema: HEAD_PROJECTION_SCHEMA,
        },
    )
    .expect_err("missing E_Reasoning must fail closed")
    .code()
    .to_string();

    let embedder_order = canonical_embedder_order();
    let signals = build_signals(embedder_order.len(), 88);
    let schema_mismatch_code = project_head(
        HeadId::Oracle,
        HeadProjectionInput {
            panel_id,
            chunk_id: &chunk_id,
            signals: &signals,
            embedder_order: &embedder_order,
            scalar_features: &BTreeMap::new(),
            schema: "head-dda-projection-v0",
        },
    )
    .expect_err("schema mismatch must fail closed")
    .code()
    .to_string();

    let empty_dda_code = project_per_head_dda_rows(
        panel_id,
        &[],
        &embedder_order,
        &BTreeMap::<ChunkId, BTreeMap<String, f32>>::new(),
    )
    .expect_err("empty DDA block must fail closed")
    .code()
    .to_string();

    Ok(EdgeCaseReport {
        missing_reasoning_code,
        schema_mismatch_code,
        empty_dda_code,
    })
}

fn sha256_file(path: &Path) -> Result<String> {
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    Ok(hex::encode(Sha256::digest(bytes)))
}
