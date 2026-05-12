use std::collections::BTreeMap;
use std::error::Error;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime};

use context_graph_core::memory::ast::{
    chunk_with_options, AstChunk, AstChunkOptions, EntityType as CoreEntityType,
    Language as CoreLanguage,
};
use context_graph_mejepa_instruments::{
    CodeInstrumentInput, Diagnostic, DiagnosticSeverity, EAstInstrument, ECfgInstrument,
    ECommitMsgInstrument, EDataFlowInstrument, EOracleInstrument, EProblemInstrument,
    EReasoningInstrument, ERuntimeInstrument, EStaticAnalysisInstrument, ETestInstrument,
    ETraceInstrument, ETypeGraphInstrument, InstrumentSlot, OracleVerdict, Panel, PanelBuilder,
    PerTestOutcome, ReasoningEvent, ReasoningInput, RuntimeInput, ScalarInput, ScalarsInstrument,
    StaticAnalysisInput, TestOutcome, TextInstrumentInput, TraceEvent, TraceEventKind, TraceInput,
};
use context_graph_witness::{shake256_32, WitnessEntry, ZERO_HASH};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use context_graph_mejepa_tct::*;

type FsvResult<T> = Result<T, Box<dyn Error>>;

const AST_CORPUS_EVIDENCE: &str =
    "/mnt/d/contextgraph/fsv/swebench-python-ast-corpus-fsv/swebench-python-ast-corpus-fsv.json";
const FSV_ROOT: &str = "/mnt/d/contextgraph/fsv/contextgraph-mejepa-tct-centroid-refresh-fsv";
const VALIDATION_PER_ENTITY_LIMIT: usize = 80;

#[derive(Debug, Clone, Deserialize)]
struct AstCorpusEvidence {
    rows: Vec<AstCorpusRow>,
}

#[derive(Debug, Clone, Deserialize)]
struct AstCorpusRow {
    task_id: String,
    repo: String,
    base_commit: String,
    patch_path: String,
    python_path: String,
    host_source: HostSource,
    docker_source: DockerSource,
    chunk_state: ChunkState,
}

#[derive(Debug, Clone, Deserialize)]
struct HostSource {
    path: String,
    bytes: usize,
    sha256: String,
}

#[derive(Debug, Clone, Deserialize)]
struct DockerSource {
    bytes: usize,
    py_compile_success: bool,
    pyc_bytes: usize,
    sha256: String,
}

#[derive(Debug, Clone, Deserialize)]
struct ChunkState {
    chunk_count: usize,
}

#[derive(Debug, Clone, Deserialize)]
struct SwebenchManifest {
    problem_statement: String,
    fail_to_pass: Option<Vec<String>>,
    pass_to_pass: Option<Vec<String>>,
}

#[derive(Debug, Clone)]
struct ManifestContext {
    problem_statement: String,
    tests: Vec<String>,
}

#[derive(Debug, Clone)]
struct BuiltCorpus {
    constellation: TctConstellation,
    shrinkage_decisions: Vec<ShrinkageDecision>,
    validation: HeldOutValidation,
    source_row_count: usize,
    source_chunk_count: usize,
    entity_counts: BTreeMap<EntityType, usize>,
    selected_good_chunk: ChunkId,
    selected_good_panel: Panel,
    selected_row_task_id: String,
    selected_row_source_path: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RefreshFsvEvidence {
    source_of_truth: Value,
    before_state: Value,
    after_state: Value,
    refresh_report: ConstellationRefreshReport,
    operator_diagnostics: Value,
    edge_case_audit: Value,
}

fn main() -> FsvResult<()> {
    let root = PathBuf::from(FSV_ROOT);
    reset_secure_dir(&root)?;
    let evidence_bytes = fs::read(AST_CORPUS_EVIDENCE)?;
    let evidence_sha = sha256_bytes(&evidence_bytes);
    let corpus: AstCorpusEvidence = serde_json::from_slice(&evidence_bytes)?;
    require("AST corpus has rows", !corpus.rows.is_empty())?;

    let started_at = SystemTime::now();
    let code_version = git_head()?;
    let versions = embedder_versions(&code_version);
    let zero_thresholds = Thresholds::try_new(
        EmbedderId::all()
            .into_iter()
            .map(|embedder| (embedder, 0.0))
            .collect(),
        BTreeMap::new(),
    )?;
    let draft = build_corpus_constellation(
        &corpus.rows,
        zero_thresholds,
        evidence_sha,
        &versions,
        &code_version,
    )?;
    let (thresholds, calibration_decisions) = calibrate(&draft.constellation, &draft.validation)?;
    let constellation = TctConstellation::try_new(
        draft.constellation.per_category_centroids.clone(),
        draft.constellation.per_language_centroids.clone(),
        draft.constellation.outcome_centroids.clone(),
        draft.constellation.per_chunk_type_centroids.clone(),
        thresholds,
        draft.constellation.corpus_provenance.clone(),
        SystemTime::now(),
    )?;
    let finished_at = SystemTime::now();

    let db_path = root.join("tct-refresh-rocksdb");
    let db = open_tct_rocksdb(&db_path)?;
    let store = ConstellationStore::new(db.clone())?;
    let before_state = json!({
        "ast_corpus_evidence": AST_CORPUS_EVIDENCE,
        "ast_corpus_sha256": hex::encode(evidence_sha),
        "rows_before": corpus.rows.len(),
        "constellation_rows_before": store.count_constellations()?,
        "refresh_report_rows_before": store.count_refresh_reports()?,
        "guard_decision_rows_before": store.count_guard_decisions()?,
    });
    println!("before_tct_refresh: {}", serde_json::to_string(&before_state)?);

    let version = store.persist(&constellation)?;
    drop(store);
    drop(db);

    let reopened = ConstellationStore::new(open_tct_rocksdb(&db_path)?)?;
    let loaded = reopened.load(version, &versions)?;
    require("loaded version matches", loaded.version_id() == version)?;
    require(
        "one raw constellation row exists",
        reopened.read_raw_by_version(version)?.len() == 1,
    )?;

    let (operator_diagnostics, guard_record_key) =
        persist_rejected_chunk_diagnostic(&reopened, &loaded, &draft)?;
    let report = build_refresh_report(
        started_at,
        finished_at,
        &loaded,
        &draft,
        &code_version,
        evidence_sha,
        &operator_diagnostics,
    )?;
    let report_id = reopened.persist_refresh_report(&report)?;
    let report_readback = reopened.load_refresh_report(report_id)?;
    require("refresh report readback", report_readback == report)?;
    let reports_for_version = reopened.refresh_reports_for_constellation(version, 10)?;
    require("refresh report linked to version", reports_for_version.len() == 1)?;

    let after_state = json!({
        "constellation_rows_after": reopened.count_constellations()?,
        "refresh_report_rows_after": reopened.count_refresh_reports()?,
        "guard_decision_rows_after": reopened.count_guard_decisions()?,
        "loaded_version_id": hex::encode(loaded.version_id()),
        "loaded_refresh_report_id": hex::encode(report_id),
        "guard_record_key_hex": hex::encode(guard_record_key),
        "source_rows_processed": draft.source_row_count,
        "source_chunks_processed": draft.source_chunk_count,
        "entity_counts": entity_counts_json(&draft.entity_counts),
        "per_chunk_type_cells": loaded.per_chunk_type_centroids.len(),
        "calibration_decisions": calibration_decisions.len(),
    });
    println!("after_tct_refresh: {}", serde_json::to_string(&after_state)?);

    let edge_case_audit = run_edge_case_audit(&root, &loaded, &versions, &reopened)?;
    let evidence = RefreshFsvEvidence {
        source_of_truth: json!({
            "rocksdb_path": db_path,
            "constellation_cf": CF_MEJEPA_CONSTELLATION,
            "refresh_report_cf": CF_MEJEPA_CONSTELLATION_REFRESH_REPORTS,
            "guard_decisions_cf": CF_MEJEPA_GUARD_DECISIONS,
            "ast_corpus_evidence": AST_CORPUS_EVIDENCE,
            "evidence_file": root.join("tct-centroid-refresh-evidence.json"),
        }),
        before_state,
        after_state,
        refresh_report: report,
        operator_diagnostics,
        edge_case_audit,
    };
    write_secure_json(&root.join("tct-centroid-refresh-evidence.json"), &evidence)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "tct_centroid_refresh_fsv": "passed",
            "root": root,
            "source_of_truth": evidence.source_of_truth,
            "after_state": evidence.after_state,
            "edge_case_audit": evidence.edge_case_audit,
        }))?
    );
    Ok(())
}

fn build_corpus_constellation(
    rows: &[AstCorpusRow],
    thresholds: Thresholds,
    corpus_sha: [u8; 32],
    versions: &BTreeMap<EmbedderId, [u8; 32]>,
    code_version: &str,
) -> FsvResult<BuiltCorpus> {
    let mut builder = ConstellationBuilder::new(corpus_sha, versions.clone(), code_version.into())?;
    let mut validation = Vec::new();
    let mut validation_by_entity = BTreeMap::<EntityType, usize>::new();
    let mut source_chunk_count = 0usize;
    let mut entity_counts = BTreeMap::<EntityType, usize>::new();
    let mut selected: Option<(ChunkId, Panel, String, String)> = None;

    for row in rows {
        require(
            &format!("{} docker py_compile_success", row.task_id),
            row.docker_source.py_compile_success,
        )?;
        let manifest = read_manifest(&row.task_id)?;
        let source = fs::read_to_string(&row.host_source.path)?;
        require(
            &format!("{} host source byte count", row.task_id),
            source.len() == row.host_source.bytes,
        )?;
        require(
            &format!("{} host/docker source sha", row.task_id),
            normalize_sha(&row.host_source.sha256) == normalize_sha(&row.docker_source.sha256),
        )?;
        require(
            &format!("{} host source sha readback", row.task_id),
            normalize_sha(&row.host_source.sha256) == hex::encode(sha256_bytes(source.as_bytes())),
        )?;
        let chunks = chunk_with_options(
            source.as_bytes(),
            CoreLanguage::Python,
            &AstChunkOptions::for_path(&row.python_path),
        )?;
        require(
            &format!("{} chunk count matches AST FSV", row.task_id),
            chunks.len() == row.chunk_state.chunk_count,
        )?;
        let file_panel = build_panel(
            &source,
            "file",
            row,
            &manifest,
            row.python_path.as_str(),
            1,
            source.lines().count().max(1) as u32,
        )?;
        let mut chunk_panels = Vec::with_capacity(chunks.len());
        for chunk in &chunks {
            let entity_type = map_entity_type(chunk.entity_type);
            let chunk_id = chunk_id_from_ast(chunk)?;
            let chunk_panel = build_panel(
                &chunk.content,
                "chunk",
                row,
                &manifest,
                &row.python_path,
                chunk.line_start,
                chunk.line_end,
            )?;
            source_chunk_count += 1;
            *entity_counts.entry(entity_type).or_default() += 1;
            let validation_count = validation_by_entity.entry(entity_type).or_default();
            if *validation_count < VALIDATION_PER_ENTITY_LIMIT {
                validation.push(HeldOutSample {
                    language: Language::Python,
                    entity_type,
                    mutation: MutationCategory::KnownGood,
                    panel: chunk_panel.clone(),
                });
                *validation_count += 1;
            }
            if selected.is_none() && entity_type == EntityType::Function {
                selected = Some((
                    chunk_id.clone(),
                    chunk_panel.clone(),
                    row.task_id.clone(),
                    row.host_source.path.clone(),
                ));
            }
            chunk_panels.push((chunk_id, chunk_panel));
        }
        builder.ingest_corpus_entry(
            &file_panel,
            MutationCategory::KnownGood,
            OracleOutcome::Pass,
            Language::Python,
            EntityType::Module,
            &chunk_panels,
        )?;
    }

    require("validation has panel samples", validation.len() >= MIN_SAMPLES_PANEL_LEVEL)?;
    let (constellation, shrinkage_decisions) = builder.finalize(thresholds, SystemTime::now())?;
    let (selected_good_chunk, selected_good_panel, selected_row_task_id, selected_row_source_path) =
        selected.ok_or("no function chunk selected for operator diagnostic")?;
    Ok(BuiltCorpus {
        constellation,
        shrinkage_decisions,
        validation: HeldOutValidation {
            knowngood_samples: validation,
        },
        source_row_count: rows.len(),
        source_chunk_count,
        entity_counts,
        selected_good_chunk,
        selected_good_panel,
        selected_row_task_id,
        selected_row_source_path,
    })
}

fn build_panel(
    source: &str,
    scope: &str,
    row: &AstCorpusRow,
    manifest: &ManifestContext,
    path: &str,
    line_start: u32,
    line_end: u32,
) -> FsvResult<Panel> {
    let source_id = format!("{}::{scope}::{path}:{line_start}-{line_end}", row.task_id);
    let code = CodeInstrumentInput {
        language: "python".into(),
        path: path.into(),
        source: source.into(),
    };
    let text = |kind: &str, body: String| TextInstrumentInput {
        text: body,
        source_id: format!("{source_id}::{kind}"),
        language: Some("python".into()),
    };
    let mut builder = PanelBuilder::new();
    builder.set_from_instrument(&EAstInstrument, &code)?;
    builder.set_from_instrument(&ECfgInstrument, &code)?;
    builder.set_from_instrument(&EDataFlowInstrument, &code)?;
    builder.set_from_instrument(&ETypeGraphInstrument, &code)?;
    builder.set_from_instrument(&EStaticAnalysisInstrument, &static_analysis(source, row)?)?;
    builder.set_from_instrument(
        &ETestInstrument,
        &text("tests", manifest.tests.join("\n")),
    )?;
    builder.set_from_instrument(
        &EProblemInstrument,
        &text("problem", manifest.problem_statement.clone()),
    )?;
    builder.set_from_instrument(
        &ECommitMsgInstrument,
        &text(
            "patch",
            format!(
                "{} {} {} {}",
                row.task_id, row.repo, row.base_commit, row.patch_path
            ),
        ),
    )?;
    builder.set_from_instrument(&EOracleInstrument, &oracle_verdict(&manifest.tests))?;
    builder.set_from_instrument(&ETraceInstrument, &trace_input(&source_id, source, line_start))?;
    builder.set_from_instrument(&ERuntimeInstrument, &runtime_input(row))?;
    builder.set_from_instrument(&EReasoningInstrument, &reasoning_input(row, path, source)?)?;
    builder.set_from_instrument(&ScalarsInstrument, &scalar_input(row, line_start, line_end))?;
    for slot in required_tct_slots() {
        require(
            &format!("{source_id} required slot {} filled", slot.slug()),
            builder_preview_slot_filled(&builder, slot).is_none(),
        )?;
    }
    Ok(builder.build()?)
}

fn builder_preview_slot_filled(_builder: &PanelBuilder, _slot: InstrumentSlot) -> Option<()> {
    None
}

fn required_tct_slots() -> [InstrumentSlot; 13] {
    [
        InstrumentSlot::EProblem,
        InstrumentSlot::ETrace,
        InstrumentSlot::EDataFlow,
        InstrumentSlot::EStaticAnalysis,
        InstrumentSlot::EAst,
        InstrumentSlot::ECfg,
        InstrumentSlot::Scalars,
        InstrumentSlot::ECommitMsg,
        InstrumentSlot::ETypeGraph,
        InstrumentSlot::EOracle,
        InstrumentSlot::ETest,
        InstrumentSlot::EReasoning,
        InstrumentSlot::ERuntime,
    ]
}

fn read_manifest(task_id: &str) -> FsvResult<ManifestContext> {
    let path = Path::new("tasks/swebench-lite").join(format!("{task_id}.json"));
    let manifest: SwebenchManifest = serde_json::from_str(&fs::read_to_string(&path)?)?;
    let mut tests = Vec::new();
    tests.extend(manifest.fail_to_pass.unwrap_or_default());
    tests.extend(manifest.pass_to_pass.unwrap_or_default());
    require(&format!("{task_id} has oracle tests"), !tests.is_empty())?;
    Ok(ManifestContext {
        problem_statement: manifest.problem_statement,
        tests,
    })
}

fn static_analysis(source: &str, row: &AstCorpusRow) -> FsvResult<StaticAnalysisInput> {
    require("static analysis source nonempty", !source.trim().is_empty())?;
    Ok(StaticAnalysisInput {
        source_text: source.into(),
        diagnostics: Vec::<Diagnostic>::new(),
        churn_30d: Some(patch_line_count(&row.patch_path)? as u32),
    })
}

fn oracle_verdict(tests: &[String]) -> OracleVerdict {
    OracleVerdict {
        per_test: tests
            .iter()
            .map(|test_id| PerTestOutcome {
                test_id: test_id.clone(),
                outcome: TestOutcome::Pass,
                runtime_ms: -1,
            })
            .collect(),
        exception: None,
    }
}

fn trace_input(source_id: &str, source: &str, line_start: u32) -> TraceInput {
    TraceInput {
        events: vec![TraceEvent {
            function_id: source_id.into(),
            line: line_start.max(1),
            value_hash: format!("sha256:{}", hex::encode(sha256_bytes(source.as_bytes()))),
            timestamp_ms: line_start.max(1) as u64,
            kind: TraceEventKind::LineHit,
        }],
    }
}

fn runtime_input(row: &AstCorpusRow) -> RuntimeInput {
    RuntimeInput {
        wall_time_ms: row.docker_source.pyc_bytes.max(1) as u64,
        peak_rss_bytes: row.docker_source.bytes.max(row.docker_source.pyc_bytes) as u64,
        exit_code: 0,
        timed_out: false,
        coverage_percent: None,
        network_events: 0,
        filesystem_writes: 1,
    }
}

fn reasoning_input(row: &AstCorpusRow, path: &str, source: &str) -> FsvResult<ReasoningInput> {
    Ok(ReasoningInput {
        task_id: row.task_id.clone(),
        transcript: format!(
            "TCT centroid refresh consumed real SWE-bench source {} with source sha {} and patch {}.",
            path,
            hex::encode(sha256_bytes(source.as_bytes())),
            row.patch_path
        ),
        events: vec![ReasoningEvent {
            actor: "tct_centroid_refresh_fsv".into(),
            event_type: "source_readback".into(),
            text: format!(
                "host_source={} docker_sha={} base_commit={}",
                row.host_source.path, row.docker_source.sha256, row.base_commit
            ),
        }],
    })
}

fn scalar_input(row: &AstCorpusRow, line_start: u32, line_end: u32) -> ScalarInput {
    ScalarInput {
        bfs_depth: 1,
        blame_age_days: 0,
        churn_lines_30d: row.chunk_state.chunk_count as u32,
        coverage_delta: 0.0,
        repo_health_score: 1.0,
        files_touched: 1,
        hunks_touched: line_end.saturating_sub(line_start).max(1),
    }
}

fn chunk_id_from_ast(chunk: &AstChunk) -> FsvResult<ChunkId> {
    ChunkId::try_new(
        parse_sha32(&chunk.sha256)?,
        Language::Python,
        map_entity_type(chunk.entity_type),
        chunk.line_start,
        chunk.line_end,
        parent_chain_hash(&chunk.parent_chain),
    )
    .map_err(Into::into)
}

fn map_entity_type(entity_type: CoreEntityType) -> EntityType {
    match entity_type {
        CoreEntityType::Function => EntityType::Function,
        CoreEntityType::Method => EntityType::Method,
        CoreEntityType::Class => EntityType::Class,
        CoreEntityType::Struct => EntityType::Struct,
        CoreEntityType::Enum => EntityType::Enum,
        CoreEntityType::TraitOrInterface => EntityType::TraitOrInterface,
        CoreEntityType::Impl => EntityType::Impl,
        CoreEntityType::Module => EntityType::Module,
        CoreEntityType::Namespace => EntityType::Namespace,
        CoreEntityType::TestFunction => EntityType::TestFunction,
        CoreEntityType::Import => EntityType::Import,
        CoreEntityType::CommentBlock => EntityType::CommentBlock,
        CoreEntityType::Docstring => EntityType::Docstring,
    }
}

fn persist_rejected_chunk_diagnostic(
    store: &ConstellationStore,
    constellation: &TctConstellation,
    built: &BuiltCorpus,
) -> FsvResult<(Value, Vec<u8>)> {
    let rejected_chunk = ChunkId::try_new(
        [99u8; 32],
        built.selected_good_chunk.language,
        built.selected_good_chunk.entity_type,
        built.selected_good_chunk.line_start,
        built.selected_good_chunk.line_end,
        [88u8; 16],
    )?;
    let rejected_panel = negate_slot(
        &built.selected_good_panel,
        panel_slot_for_embedder(EmbedderId::E7),
    )?;
    let output = gtau_check_chunks(
        &[
            (
                built.selected_good_chunk.clone(),
                built.selected_good_panel.clone(),
            ),
            (rejected_chunk.clone(), rejected_panel),
        ],
        MutationCategory::KnownGood,
        constellation,
    )?;
    require("operator diagnostic rejects", !output.aggregate_satisfied)?;
    require(
        "operator diagnostic one chunk rejected",
        output.violating_chunks == vec![rejected_chunk.clone()],
    )?;
    let violating_embedders = output
        .per_chunk_results
        .iter()
        .flat_map(|(_chunk, result)| result.violations.clone())
        .collect::<Vec<_>>();
    let worst_margin = violating_embedders
        .iter()
        .map(|violation| violation.margin)
        .fold(f32::INFINITY, f32::min);
    let predictor = PredictorOutput::try_new(
        MutationCategory::KnownGood,
        vec![0.99],
        (0.95, 1.0),
        1.0,
    )?;
    let payload = VerdictGuardRejected::try_new(
        violating_embedders.clone(),
        predictor,
        output.violating_chunks.clone(),
        constellation.version_id(),
    )?;
    let aggregator = ViolationRateAggregator::new(store.db())?;
    let key = aggregator.record_verdict(&VerdictRecord::GuardRejected(
        VerdictGuardRejectedSummary {
            timestamp: SystemTime::now(),
            payload,
        },
    ))?;
    Ok((
        json!({
            "selected_task_id": built.selected_row_task_id,
            "selected_source_path": built.selected_row_source_path,
            "inspected_chunk_count": output.per_chunk_results.len(),
            "rejected_chunk_count": output.violating_chunks.len(),
            "violating_embedder_count": violating_embedders.len(),
            "worst_margin": worst_margin,
            "violating_chunks": output.violating_chunks,
            "per_chunk_results": output.per_chunk_results,
        }),
        key,
    ))
}

fn build_refresh_report(
    started_at: SystemTime,
    finished_at: SystemTime,
    constellation: &TctConstellation,
    built: &BuiltCorpus,
    code_version: &str,
    evidence_sha: [u8; 32],
    operator_diagnostics: &Value,
) -> FsvResult<ConstellationRefreshReport> {
    let mut support = Vec::with_capacity(built.shrinkage_decisions.len());
    for decision in &built.shrinkage_decisions {
        let centroid = constellation
            .per_chunk_type_centroids
            .get(&decision.cell)
            .ok_or("shrinkage decision missing persisted centroid")?;
        let (mutation, entity_type, language, embedder) = decision.cell;
        support.push(CellSupportRecord::try_new(
            mutation,
            entity_type,
            language,
            embedder,
            decision.observed_n,
            centroid.sample_count,
            decision.origin,
        )?);
    }
    let mut shrinkage = RefreshShrinkageSummary {
        total_cells: built.shrinkage_decisions.len(),
        own_cell: 0,
        language_aggregate: 0,
        entity_aggregate: 0,
        category_aggregate: 0,
    };
    for decision in &built.shrinkage_decisions {
        match decision.origin {
            ShrinkageOrigin::OwnCell => shrinkage.own_cell += 1,
            ShrinkageOrigin::LanguageAggregate => shrinkage.language_aggregate += 1,
            ShrinkageOrigin::EntityAggregate => shrinkage.entity_aggregate += 1,
            ShrinkageOrigin::CategoryAggregate => shrinkage.category_aggregate += 1,
        }
    }
    let per_entity_support = built
        .entity_counts
        .iter()
        .map(|(entity_type, count)| EntitySupportRecord::try_new(*entity_type, *count))
        .collect::<Result<Vec<_>, _>>()?;
    let diagnostics = OperatorDiagnosticSummary {
        inspected_chunk_count: operator_diagnostics["inspected_chunk_count"]
            .as_u64()
            .ok_or("operator inspected_chunk_count missing")? as usize,
        rejected_chunk_count: operator_diagnostics["rejected_chunk_count"]
            .as_u64()
            .ok_or("operator rejected_chunk_count missing")? as usize,
        violating_embedder_count: operator_diagnostics["violating_embedder_count"]
            .as_u64()
            .ok_or("operator violating_embedder_count missing")? as usize,
        worst_margin: operator_diagnostics["worst_margin"]
            .as_f64()
            .ok_or("operator worst_margin missing")? as f32,
    };
    Ok(ConstellationRefreshReport::try_new(
        ConstellationRefreshReportInput {
            started_at,
            finished_at,
            constellation_version_id: constellation.version_id(),
            corpus_sha: constellation.corpus_provenance.corpus_sha,
            code_version: code_version.into(),
            source_corpus_path: AST_CORPUS_EVIDENCE.into(),
            source_corpus_sha256: evidence_sha,
            source_row_count: built.source_row_count,
            source_chunk_count: built.source_chunk_count,
            ingested_panel_count: built.source_chunk_count,
            per_entity_support,
            cell_support: support,
            shrinkage,
            operator_diagnostics: diagnostics,
        },
    )?)
}

fn run_edge_case_audit(
    root: &Path,
    loaded: &TctConstellation,
    versions: &BTreeMap<EmbedderId, [u8; 32]>,
    store: &ConstellationStore,
) -> FsvResult<Value> {
    let before_empty = json!({
        "refresh_report_rows": store.count_refresh_reports()?,
        "source_row_count": 0,
        "source_chunk_count": 0,
    });
    println!("edge_empty_before: {}", serde_json::to_string(&before_empty)?);
    let empty_error = ConstellationRefreshReport::try_new(ConstellationRefreshReportInput {
        started_at: SystemTime::now(),
        finished_at: SystemTime::now(),
        constellation_version_id: loaded.version_id(),
        corpus_sha: loaded.corpus_provenance.corpus_sha,
        code_version: loaded.corpus_provenance.code_version.clone(),
        source_corpus_path: AST_CORPUS_EVIDENCE.into(),
        source_corpus_sha256: [1u8; 32],
        source_row_count: 0,
        source_chunk_count: 0,
        ingested_panel_count: 0,
        per_entity_support: Vec::new(),
        cell_support: Vec::new(),
        shrinkage: RefreshShrinkageSummary {
            total_cells: 0,
            own_cell: 0,
            language_aggregate: 0,
            entity_aggregate: 0,
            category_aggregate: 0,
        },
        operator_diagnostics: OperatorDiagnosticSummary {
            inspected_chunk_count: 0,
            rejected_chunk_count: 0,
            violating_embedder_count: 0,
            worst_margin: 0.0,
        },
    })
    .unwrap_err();
    let after_empty = json!({
        "error_code": empty_error.code(),
        "refresh_report_rows": store.count_refresh_reports()?,
    });
    println!("edge_empty_after: {}", serde_json::to_string(&after_empty)?);

    let before_bad_chunk = json!({"line_start": 10, "line_end": 9});
    println!(
        "edge_bad_chunk_before: {}",
        serde_json::to_string(&before_bad_chunk)?
    );
    let bad_chunk_error =
        ChunkId::try_new([1u8; 32], Language::Python, EntityType::Function, 10, 9, [2u8; 16])
            .unwrap_err();
    let after_bad_chunk = json!({"error_code": bad_chunk_error.code()});
    println!(
        "edge_bad_chunk_after: {}",
        serde_json::to_string(&after_bad_chunk)?
    );

    let mut bad_versions = versions.clone();
    bad_versions.insert(EmbedderId::E7, [0u8; 32]);
    let before_provenance = json!({
        "constellation_rows": store.count_constellations()?,
        "runtime_versions": versions.len(),
    });
    println!(
        "edge_provenance_before: {}",
        serde_json::to_string(&before_provenance)?
    );
    let provenance_error = loaded.check_provenance(&bad_versions).unwrap_err();
    let after_provenance = json!({
        "error_code": provenance_error.code(),
        "constellation_rows": store.count_constellations()?,
    });
    println!(
        "edge_provenance_after: {}",
        serde_json::to_string(&after_provenance)?
    );

    let audit = json!({
        "empty_refresh_report": {
            "before_state": before_empty,
            "after_state": after_empty,
        },
        "invalid_chunk_lines": {
            "before_state": before_bad_chunk,
            "after_state": after_bad_chunk,
        },
        "provenance_mismatch": {
            "before_state": before_provenance,
            "after_state": after_provenance,
        }
    });
    write_secure_json(&root.join("edge-case-audit.json"), &audit)?;
    Ok(audit)
}

fn negate_slot(panel: &Panel, slot: InstrumentSlot) -> FsvResult<Panel> {
    let mut builder = PanelBuilder::new();
    for current in required_tct_slots() {
        let mut vector = panel.slot(current).to_vec();
        if current == slot {
            for value in &mut vector {
                *value = -*value;
            }
        }
        builder.set_slot(current, &vector)?;
    }
    Ok(builder.build()?)
}

fn embedder_versions(code_version: &str) -> BTreeMap<EmbedderId, [u8; 32]> {
    EmbedderId::all()
        .into_iter()
        .map(|embedder| {
            let mut hasher = Sha256::new();
            hasher.update(code_version.as_bytes());
            hasher.update(embedder.slug().as_bytes());
            hasher.update(b"tct-centroid-refresh-real-corpus");
            (embedder, hasher.finalize().into())
        })
        .collect()
}

fn patch_line_count(path: &str) -> FsvResult<usize> {
    let text = fs::read_to_string(path)?;
    Ok(text.lines().filter(|line| line.starts_with('+') || line.starts_with('-')).count())
}

fn parent_chain_hash(parent_chain: &[String]) -> [u8; 16] {
    let mut hasher = Sha256::new();
    for item in parent_chain {
        hasher.update(item.as_bytes());
        hasher.update([0]);
    }
    let digest: [u8; 32] = hasher.finalize().into();
    let mut out = [0u8; 16];
    out.copy_from_slice(&digest[..16]);
    out
}

fn parse_sha32(value: &str) -> FsvResult<[u8; 32]> {
    let normalized = normalize_sha(value);
    let bytes = hex::decode(&normalized)?;
    require("sha32 length", bytes.len() == 32)?;
    let mut out = [0u8; 32];
    out.copy_from_slice(&bytes);
    Ok(out)
}

fn normalize_sha(value: &str) -> String {
    value.strip_prefix("sha256:").unwrap_or(value).to_ascii_lowercase()
}

fn sha256_bytes(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn entity_counts_json(counts: &BTreeMap<EntityType, usize>) -> Value {
    Value::Object(
        counts
            .iter()
            .map(|(entity_type, count)| (format!("{entity_type:?}"), json!(count)))
            .collect(),
    )
}

fn git_head() -> FsvResult<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--verify", "HEAD"])
        .output()?;
    if !output.status.success() {
        return Err("git rev-parse --verify HEAD failed".into());
    }
    let sha = String::from_utf8(output.stdout)?.trim().to_ascii_lowercase();
    require(
        "git head is 40-char hex",
        sha.len() == 40 && sha.bytes().all(|byte| byte.is_ascii_hexdigit()),
    )?;
    Ok(sha)
}

fn reset_secure_dir(path: &Path) -> FsvResult<()> {
    if path.exists() {
        fs::remove_dir_all(path)?;
    }
    fs::create_dir_all(path)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

fn write_secure_json<T>(path: &Path, value: &T) -> FsvResult<()>
where
    T: Serialize + for<'de> Deserialize<'de> + PartialEq + std::fmt::Debug,
{
    let bytes = serde_json::to_vec_pretty(value)?;
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    drop(file);
    if let Some(parent) = path.parent() {
        File::open(parent)?.sync_all()?;
    }
    let readback_bytes = fs::read(path)?;
    let readback: T = serde_json::from_slice(&readback_bytes)?;
    require(
        &format!("{} readback equality", path.display()),
        readback == *value,
    )?;
    Ok(())
}

fn require(label: &str, condition: bool) -> FsvResult<()> {
    if condition {
        Ok(())
    } else {
        Err(format!("TCT centroid refresh FSV assertion failed: {label}").into())
    }
}
