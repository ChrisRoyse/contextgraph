//! TASK-EK-003 mincut panel FSV.
//!
//! Proves the structural-hole panel across inline, DDA embedder, TCT
//! constellation, and failure-fingerprint graph sources, with durable
//! CF_MEJEPA_MINCUT_REPORTS readback and fail-closed edge cases.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{bail, Context, Result};
use context_graph_mejepa::eval::MutationCategory as EvalMutationCategory;
use context_graph_mejepa::{
    count_cf, fingerprint_key, mejepa_mincut_panel, open_mincut_rocksdb, read_mincut_report,
    write_mincut_report_sync_readback, ChunkId as InferChunkId, DdaSignals, FailureModeClass,
    FailureShapeFingerprint, FingerprintCalibrationState, FingerprintConfidence, FingerprintKind,
    MincutOptions, MincutReport, OracleOutcome as InferOracleOutcome, PanelGraphSource, PanelId,
    PanelSimilarityWeight, MEJEPA_MINCUT_DEGENERATE,
};
use context_graph_mejepa_cf::{
    CF_MEJEPA_DDA_SIGNALS, CF_MEJEPA_FAILURE_FINGERPRINTS, CF_MEJEPA_MINCUT_REPORTS,
};
use context_graph_mejepa_instruments::{InstrumentSlot, Panel, PanelBuilder};
use context_graph_mejepa_tct::{
    ConstellationBuilder, ConstellationStore, EmbedderId as TctEmbedderId,
    EntityType as TctEntityType, Language as TctLanguage, MutationCategory as TctMutationCategory,
    OracleOutcome as TctOracleOutcome, Thresholds,
};
use context_graph_mincut::weights_from_edges;
use rocksdb::{WriteOptions, DB};
use serde::{Deserialize, Serialize};

const FSV_ROOT: &str = "/zfs/archive/contextgraph/fsv/task-ek-003-mincut-panel-fsv";
const MCP_TOOL_NAME: &str = "mcp__cgreality__mejepa_mincut_panel";
const CLI_TOOL_NAME: &str = "context-graph-cli mejepa mincut-panel";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SourceEvidence {
    source_kind: String,
    report_id: String,
    graph_id: String,
    cut_value: f32,
    conductance: f32,
    edge_count: usize,
    source_row_count: usize,
    partition_left_len: usize,
    partition_right_len: usize,
    direction_count: usize,
    persisted_readback_equal: bool,
    warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TaskEk003Fsv {
    output_path: String,
    db_path: String,
    started_at_unix_ms: i64,
    mcp_tool_name: String,
    cli_tool_name: String,
    mincut_report_cf: String,
    source_evidence: Vec<SourceEvidence>,
    paper_fig_1_cut_is_four: bool,
    all_zero_degenerate_warning: bool,
    disconnected_fails_closed: bool,
    two_node_cut_is_one: bool,
    self_loop_fails_closed: bool,
    cf_row_count_after_reopen: u64,
    persisted_report_survived_reopen: bool,
    json_readback_equal: bool,
    all_passed: bool,
}

fn main() -> Result<()> {
    let started_at_unix_ms = unix_ms()?;
    let run_root = PathBuf::from(FSV_ROOT).join(format!("run-{started_at_unix_ms}"));
    let db_path = run_root.join("rocksdb");
    let output_path = run_root.join("mincut_panel_fsv.json");
    fs::create_dir_all(&run_root).context("create TASK-EK-003 FSV root")?;

    let db = open_mincut_rocksdb(&db_path).context("open mincut RocksDB")?;
    seed_dda_graph(db.as_ref())?;
    let constellation_version = seed_constellation_graph(db.clone())?;
    seed_fingerprint_graph(db.as_ref(), started_at_unix_ms)?;

    let paper = run_and_persist(
        db.as_ref(),
        PanelGraphSource::InlineWeightedGraph {
            graph_id: "stoer-wagner-paper-fig-1".to_string(),
            node_ids: (0..8).map(|idx| format!("v{idx}")).collect(),
            weights: paper_fig_1_weights()?,
        },
        started_at_unix_ms + 1,
    )?;
    let dda = run_and_persist(
        db.as_ref(),
        PanelGraphSource::EmbedderSimilarity {
            panel_id: Some(PanelId([0x31; 32])),
            weight: PanelSimilarityWeight::Cosine,
            max_rows: 64,
        },
        started_at_unix_ms + 2,
    )?;
    let constellation = run_and_persist(
        db.as_ref(),
        PanelGraphSource::ConstellationInternal {
            version_id: Some(hex::encode(constellation_version)),
            identity_id: "known_good".to_string(),
            modalities: vec!["e2".to_string(), "e3".to_string(), "e4".to_string()],
        },
        started_at_unix_ms + 3,
    )?;
    let fingerprint = run_and_persist(
        db.as_ref(),
        PanelGraphSource::FailureFingerprintGraph {
            lookback_window: 24 * 60 * 60,
            limit: 4,
        },
        started_at_unix_ms + 4,
    )?;
    let zero = run_and_persist(
        db.as_ref(),
        PanelGraphSource::InlineWeightedGraph {
            graph_id: "all-zero-degenerate".to_string(),
            node_ids: vec!["a".to_string(), "b".to_string(), "c".to_string()],
            weights: vec![
                vec![0.0, 0.0, 0.0],
                vec![0.0, 0.0, 0.0],
                vec![0.0, 0.0, 0.0],
            ],
        },
        started_at_unix_ms + 5,
    )?;
    let two_node = run_and_persist(
        db.as_ref(),
        PanelGraphSource::InlineWeightedGraph {
            graph_id: "two-node".to_string(),
            node_ids: vec!["left".to_string(), "right".to_string()],
            weights: vec![vec![0.0, 1.0], vec![1.0, 0.0]],
        },
        started_at_unix_ms + 6,
    )?;

    let disconnected_fails_closed = mejepa_mincut_panel(
        Some(db.as_ref()),
        PanelGraphSource::InlineWeightedGraph {
            graph_id: "disconnected-nonzero".to_string(),
            node_ids: (0..4).map(|idx| format!("d{idx}")).collect(),
            weights: weights_from_edges(4, [(0, 1, 1.0), (2, 3, 1.0)])?
                .chunks(4)
                .map(|row| row.iter().map(|value| *value as f32).collect())
                .collect(),
        },
        MincutOptions::default(),
        started_at_unix_ms + 7,
    )
    .err()
    .map(|err| err.to_string().contains("MINCUT_GRAPH_DISCONNECTED"))
    .unwrap_or(false);

    let self_loop_fails_closed = mejepa_mincut_panel(
        Some(db.as_ref()),
        PanelGraphSource::InlineWeightedGraph {
            graph_id: "self-loop".to_string(),
            node_ids: vec!["a".to_string(), "b".to_string()],
            weights: vec![vec![0.1, 1.0], vec![1.0, 0.0]],
        },
        MincutOptions::default(),
        started_at_unix_ms + 8,
    )
    .err()
    .map(|err| err.to_string().contains("self-loop weights must be zero"))
    .unwrap_or(false);

    drop(db);
    let reopened = open_mincut_rocksdb(&db_path).context("reopen mincut RocksDB")?;
    let cf_row_count_after_reopen = count_cf(reopened.as_ref(), CF_MEJEPA_MINCUT_REPORTS)?;
    let paper_readback = read_mincut_report(reopened.as_ref(), &paper.report_id)?
        .context("paper mincut report missing after reopen")?;
    let persisted_report_survived_reopen = paper_readback == paper;

    let source_evidence = vec![
        evidence(&paper, true),
        evidence(&dda, true),
        evidence(&constellation, true),
        evidence(&fingerprint, true),
        evidence(&zero, true),
        evidence(&two_node, true),
    ];
    let paper_fig_1_cut_is_four = (paper.cut_value - 4.0).abs() <= f32::EPSILON;
    let all_zero_degenerate_warning = zero.cut_value == 0.0
        && zero
            .warnings
            .contains(&MEJEPA_MINCUT_DEGENERATE.to_string());
    let two_node_cut_is_one = (two_node.cut_value - 1.0).abs() <= f32::EPSILON;
    let all_passed = paper_fig_1_cut_is_four
        && dda.graph_source_kind == "embedder_similarity"
        && constellation.graph_source_kind == "constellation_internal"
        && fingerprint.graph_source_kind == "failure_fingerprint_graph"
        && all_zero_degenerate_warning
        && disconnected_fails_closed
        && two_node_cut_is_one
        && self_loop_fails_closed
        && cf_row_count_after_reopen == source_evidence.len() as u64
        && persisted_report_survived_reopen
        && source_evidence
            .iter()
            .all(|entry| entry.persisted_readback_equal && entry.direction_count > 0);

    let mut output = TaskEk003Fsv {
        output_path: output_path.display().to_string(),
        db_path: db_path.display().to_string(),
        started_at_unix_ms,
        mcp_tool_name: MCP_TOOL_NAME.to_string(),
        cli_tool_name: CLI_TOOL_NAME.to_string(),
        mincut_report_cf: CF_MEJEPA_MINCUT_REPORTS.to_string(),
        source_evidence,
        paper_fig_1_cut_is_four,
        all_zero_degenerate_warning,
        disconnected_fails_closed,
        two_node_cut_is_one,
        self_loop_fails_closed,
        cf_row_count_after_reopen,
        persisted_report_survived_reopen,
        json_readback_equal: false,
        all_passed,
    };
    if !output.all_passed {
        bail!("TASK-EK-003 FSV assertions failed: {output:#?}");
    }
    let bytes = serde_json::to_vec_pretty(&output).context("serialize FSV output")?;
    fs::write(&output_path, &bytes).context("write FSV output")?;
    let readback = fs::read(&output_path).context("read FSV output")?;
    let mut decoded: TaskEk003Fsv =
        serde_json::from_slice(&readback).context("decode FSV output")?;
    decoded.json_readback_equal = output.json_readback_equal;
    output.json_readback_equal = readback == bytes && decoded == output;
    let bytes = serde_json::to_vec_pretty(&output).context("serialize final FSV output")?;
    fs::write(&output_path, &bytes).context("write final FSV output")?;
    let final_readback: TaskEk003Fsv =
        serde_json::from_slice(&fs::read(&output_path).context("read final FSV output")?)
            .context("decode final FSV output")?;
    if final_readback != output || !output.json_readback_equal {
        bail!("TASK-EK-003 FSV JSON readback mismatch");
    }
    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}

fn run_and_persist(db: &DB, source: PanelGraphSource, ts: i64) -> Result<MincutReport> {
    let report = mejepa_mincut_panel(Some(db), source, MincutOptions::default(), ts)?;
    write_mincut_report_sync_readback(db, &report)?;
    let readback =
        read_mincut_report(db, &report.report_id)?.context("mincut report missing after write")?;
    if readback != report {
        bail!("mincut report readback mismatch: {}", report.report_id);
    }
    Ok(report)
}

fn evidence(report: &MincutReport, persisted_readback_equal: bool) -> SourceEvidence {
    SourceEvidence {
        source_kind: report.graph_source_kind.clone(),
        report_id: report.report_id.clone(),
        graph_id: report.graph_id.clone(),
        cut_value: report.cut_value,
        conductance: report.conductance,
        edge_count: report.edge_count,
        source_row_count: report.source_row_count,
        partition_left_len: report.partition.left.len(),
        partition_right_len: report.partition.right.len(),
        direction_count: report.recommended_addition_directions.len(),
        persisted_readback_equal,
        warnings: report.warnings.clone(),
    }
}

fn paper_fig_1_weights() -> Result<Vec<Vec<f32>>> {
    Ok(weights_from_edges(
        8,
        [
            (0, 1, 2.0),
            (0, 4, 3.0),
            (1, 2, 3.0),
            (1, 4, 2.0),
            (1, 5, 2.0),
            (2, 3, 4.0),
            (2, 6, 2.0),
            (3, 6, 2.0),
            (3, 7, 2.0),
            (4, 5, 3.0),
            (5, 6, 1.0),
            (6, 7, 3.0),
        ],
    )?
    .chunks(8)
    .map(|row| row.iter().map(|value| *value as f32).collect())
    .collect())
}

fn seed_dda_graph(db: &DB) -> Result<()> {
    let signals = DdaSignals::try_new(DdaSignals {
        per_embedder_cosine: vec![0.95, 0.94, 0.93, 0.92],
        pairwise_cosine_upper: vec![1.0, -0.90, -0.90, -0.90, -0.90, 1.0],
        pairwise_mi_upper: vec![1.0, 0.05, 0.05, 0.05, 0.05, 1.0],
        blind_spot_z_scores: vec![0.0; 6],
    })?;
    let key = bincode::serialize(&(PanelId([0x31; 32]), InferChunkId("chunk-dda".to_string())))?;
    put_cf_sync(
        db,
        CF_MEJEPA_DDA_SIGNALS,
        &key,
        &serde_json::to_vec(&signals)?,
    )?;
    Ok(())
}

fn seed_constellation_graph(db: std::sync::Arc<DB>) -> Result<[u8; 32]> {
    let store = ConstellationStore::new(db).context("open TCT constellation store")?;
    let constellation = build_constellation()?;
    store
        .persist(&constellation)
        .context("persist synthetic TCT constellation")
}

fn build_constellation() -> Result<context_graph_mejepa_tct::TctConstellation> {
    let mut builder = ConstellationBuilder::new(
        [0x44; 32],
        TctEmbedderId::all()
            .into_iter()
            .enumerate()
            .map(|(idx, embedder)| (embedder, [idx as u8 + 1; 32]))
            .collect(),
        "a".repeat(40),
    )?;
    let panel = base_panel(0.0)?;
    let chunk = context_graph_mejepa_tct::ChunkId::try_new(
        [0x55; 32],
        TctLanguage::Python,
        TctEntityType::Function,
        10,
        20,
        [0x56; 16],
    )?;
    for idx in 0..5 {
        let shifted = base_panel(idx as f32 * 0.001)?;
        builder.ingest_corpus_entry(
            &shifted,
            TctMutationCategory::KnownGood,
            TctOracleOutcome::Pass,
            TctLanguage::Python,
            TctEntityType::Function,
            &[(chunk.clone(), panel.clone())],
        )?;
    }
    let thresholds = Thresholds::try_new(
        TctEmbedderId::all()
            .into_iter()
            .map(|embedder| (embedder, 0.0))
            .collect(),
        BTreeMap::new(),
    )?;
    let (constellation, _decisions) = builder.finalize(thresholds, SystemTime::now())?;
    Ok(constellation)
}

fn base_panel(shift: f32) -> Result<Panel> {
    let mut builder = PanelBuilder::new();
    for slot in InstrumentSlot::all() {
        let vector = (0..slot.dim())
            .map(|idx| {
                let phase = (slot.offset() + idx + 1) as f32 * 0.013 + shift;
                phase.sin() * 0.25 + phase.cos() * 0.05 + 0.01
            })
            .collect::<Vec<_>>();
        builder.set_slot(slot, &vector)?;
    }
    builder.require_slots(&InstrumentSlot::all())?;
    Ok(builder.build()?)
}

fn seed_fingerprint_graph(db: &DB, frozen_at_unix_ms: i64) -> Result<()> {
    let left = fixture_fingerprint(
        "fingerprint-left",
        vec![1.0, 0.0],
        EvalMutationCategory::OffByOne,
        frozen_at_unix_ms,
    )?;
    let right = fixture_fingerprint(
        "fingerprint-right",
        vec![0.0, 1.0],
        EvalMutationCategory::WrongFile,
        frozen_at_unix_ms + 1,
    )?;
    put_cf_sync(
        db,
        CF_MEJEPA_FAILURE_FINGERPRINTS,
        &fingerprint_key(left.fingerprint_id),
        &bincode::serialize(&left)?,
    )?;
    put_cf_sync(
        db,
        CF_MEJEPA_FAILURE_FINGERPRINTS,
        &fingerprint_key(right.fingerprint_id),
        &bincode::serialize(&right)?,
    )?;
    Ok(())
}

fn fixture_fingerprint(
    repo: &str,
    centroid: Vec<f32>,
    mutation_category: EvalMutationCategory,
    frozen_at_unix_ms: i64,
) -> Result<FailureShapeFingerprint> {
    let kind = FingerprintKind::KnownBad {
        repo: repo.to_string(),
        mutation_category,
        failure_mode: FailureModeClass::OffByOne,
        exception_class: None,
    };
    let source_corpus = "task-ek-003-fsv";
    let fingerprint_id = FailureShapeFingerprint::canonical_id(&kind, source_corpus)?;
    let embedder = context_graph_mejepa::EmbedderId("e1".to_string());
    let fingerprint = FailureShapeFingerprint {
        schema_version: context_graph_mejepa::FAILURE_FINGERPRINT_SCHEMA_VERSION,
        fingerprint_id,
        name: kind.class_slug(),
        kind,
        source_corpus: source_corpus.to_string(),
        source_manifest_sha256: Some([0x5a; 32]),
        centroid_by_embedder: BTreeMap::from([(embedder.clone(), centroid)]),
        variance_by_embedder: BTreeMap::from([(embedder.clone(), 0.01)]),
        tau_by_embedder: BTreeMap::from([(embedder, 0.90)]),
        pairwise_cosine: Vec::new(),
        pairwise_mutual_information: Vec::new(),
        reference_chunks: vec![InferChunkId(format!("chunk-{repo}"))],
        n_references: 1,
        oracle_outcome: Some(InferOracleOutcome::Fail),
        is_canonical: true,
        frozen_at_unix_ms,
        confidence: FingerprintConfidence {
            classification_accuracy: Some(1.0),
            classification_precision: Some(1.0),
            unknown_recall: None,
            calibration_observations: 2,
            calibration_state: FingerprintCalibrationState::Calibrated,
        },
    };
    fingerprint.validate()?;
    Ok(fingerprint)
}

fn put_cf_sync(db: &DB, cf_name: &'static str, key: &[u8], value: &[u8]) -> Result<()> {
    let cf = db
        .cf_handle(cf_name)
        .with_context(|| format!("missing column family {cf_name}"))?;
    let mut opts = WriteOptions::default();
    opts.set_sync(true);
    db.put_cf_opt(cf, key, value, &opts)
        .with_context(|| format!("put {cf_name}"))?;
    db.flush_cf(cf)
        .with_context(|| format!("flush {cf_name}"))?;
    let readback = db
        .get_cf(cf, key)
        .with_context(|| format!("readback {cf_name}"))?
        .with_context(|| format!("missing readback row in {cf_name}"))?;
    if readback != value {
        bail!("{cf_name} readback bytes mismatch");
    }
    Ok(())
}

fn unix_ms() -> Result<i64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock before UNIX epoch")?
        .as_millis()
        .try_into()
        .context("timestamp overflow")
}
