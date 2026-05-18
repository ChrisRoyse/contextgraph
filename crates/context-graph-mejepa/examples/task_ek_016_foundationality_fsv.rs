use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{ensure, Context, Result};
use context_graph_mejepa::heal::{
    consume_persisted_chunk_foundationality_fisher, list_foundationality_weighted_fisher_reports,
    HealRocksStore, FOUNDATIONALITY_FISHER_MULTIPLIER_SOURCE,
};
use context_graph_mejepa::{
    apply_foundationality_fisher_multiplier, bedrock_consistency_for_chunks,
    compression_aggressiveness_from_foundationality, compute_chunk_foundationality, count_cf,
    current_unix_ms, extract_python_chunk_dependency_edges, open_infer_rocksdb,
    persist_chunk_foundationality_report_sync_readback, read_all_chunk_dependency_edges,
    read_all_chunk_foundationality_scores, read_chunk_foundationality_score, write_json_0600,
    ChunkDependencyEdge, ChunkFoundationalityConfig, PythonChunkSource,
    MEJEPA_FOUNDATIONALITY_GRAPH_DISCONNECTED,
};
use context_graph_mejepa_cf::{CF_MEJEPA_CHUNK_DEPENDENCY_GRAPH, CF_MEJEPA_CHUNK_FOUNDATIONALITY};
use context_graph_mejepa_train::cli::load_corpus_index_dataset;
use context_graph_mejepa_train::config::TrainingConfig;
use context_graph_mejepa_train::eval::holdout::HoldoutExample;
use context_graph_mejepa_train::eval::{Lang, MutationCategory};
use context_graph_mejepa_train::sampler::{BatchSampler, PatchSimilarityGraph};
use serde_json::json;
use sha2::{Digest, Sha256};

const FSV_ROOT: &str = "/mnt/d/contextgraph/fsv/task-ek-016-foundationality-fsv";
const SWE_300X8_CORPUS_DIR: &str =
    "/mnt/d/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511";
const SWE_300X8_INDEX_PATH: &str =
    "/mnt/d/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511/index.json";

fn main() -> Result<()> {
    let started_at_unix_ms = current_unix_ms();
    let run_root = PathBuf::from(FSV_ROOT).join(format!("run-{started_at_unix_ms}"));
    let db_path = run_root.join("infer-rocksdb");
    let source_db_path = run_root.join("python-source-rocksdb");
    let production_fisher_db_path = run_root.join("production-fisher-rocksdb");
    let lambda_fisher_ablation_db_path = run_root.join("lambda-fisher-ablation-rocksdb");
    let lambda_found_ablation_db_path = run_root.join("lambda-found-300x8-rocksdb");
    let fsv_path = run_root.join("foundationality_fsv.json");
    let canonical_path = PathBuf::from(FSV_ROOT).join("foundationality_fsv.json");
    fs::create_dir_all(&run_root).context("create TASK-EK-016 FSV dir")?;

    let edges = fixture_edges();
    let report = compute_chunk_foundationality(
        &edges,
        started_at_unix_ms,
        ChunkFoundationalityConfig::default(),
    )
    .context("compute chunk foundationality")?;
    let top = report
        .scores
        .first()
        .context("foundationality report has scores")?;
    ensure!(
        top.chunk_id == "core:contract",
        "expected core:contract at rank 1"
    );
    ensure!(
        (top.foundationality_score - 1.0).abs() < 1e-6,
        "expected normalized bedrock score 1.0"
    );

    let db = open_infer_rocksdb(&db_path).context("open FSV RocksDB")?;
    let edge_count_before = count_cf(db.as_ref(), CF_MEJEPA_CHUNK_DEPENDENCY_GRAPH)?;
    let score_count_before = count_cf(db.as_ref(), CF_MEJEPA_CHUNK_FOUNDATIONALITY)?;
    persist_chunk_foundationality_report_sync_readback(db.as_ref(), &edges, &report)
        .context("persist foundationality report")?;
    let direct_edges = read_all_chunk_dependency_edges(db.as_ref())?;
    let direct_scores = read_all_chunk_foundationality_scores(db.as_ref())?;
    let direct_core = read_chunk_foundationality_score(db.as_ref(), "core:contract")?
        .context("direct core score")?;

    let high_report = bedrock_consistency_for_chunks(
        db.as_ref(),
        &["leaf:test_b".to_string(), "core:contract".to_string()],
        0.75,
        5,
    )
    .context("high bedrock report")?;
    let low_chunk_id = report
        .scores
        .last()
        .context("lowest score")?
        .chunk_id
        .clone();
    let low_report =
        bedrock_consistency_for_chunks(db.as_ref(), std::slice::from_ref(&low_chunk_id), 0.75, 5)
            .context("low bedrock report")?;
    drop(db);

    let reopened = open_infer_rocksdb(&db_path).context("reopen FSV RocksDB")?;
    let reopened_edges = read_all_chunk_dependency_edges(reopened.as_ref())?;
    let reopened_scores = read_all_chunk_foundationality_scores(reopened.as_ref())?;
    let reopened_core = read_chunk_foundationality_score(reopened.as_ref(), "core:contract")?
        .context("reopened core score")?;
    let edge_count_after_reopen = count_cf(reopened.as_ref(), CF_MEJEPA_CHUNK_DEPENDENCY_GRAPH)?;
    let score_count_after_reopen = count_cf(reopened.as_ref(), CF_MEJEPA_CHUNK_FOUNDATIONALITY)?;

    let ablation = foundationality_sampler_ablation(top.foundationality_score)?;
    let weighted_fisher =
        apply_foundationality_fisher_multiplier(2.0, top.foundationality_score, 1.0)?;
    let leaf_score = report
        .scores
        .iter()
        .find(|score| score.chunk_id == low_chunk_id)
        .context("leaf score lookup")?
        .foundationality_score;
    let leaf_fisher = apply_foundationality_fisher_multiplier(2.0, leaf_score, 1.0)?;
    let core_compression =
        compression_aggressiveness_from_foundationality(top.foundationality_score)?;
    let leaf_compression = compression_aggressiveness_from_foundationality(leaf_score)?;

    let production_fisher_store =
        HealRocksStore::open(&production_fisher_db_path).context("open production Fisher store")?;
    let fisher_foundationality_report = compute_chunk_foundationality(
        &edges,
        started_at_unix_ms,
        ChunkFoundationalityConfig {
            lambda_fisher: 2.0,
            ..ChunkFoundationalityConfig::default()
        },
    )
    .context("compute lambda_fisher=2 foundationality report")?;
    let fisher_store_db = production_fisher_store.db();
    persist_chunk_foundationality_report_sync_readback(
        fisher_store_db.as_ref(),
        &edges,
        &fisher_foundationality_report,
    )
    .context("persist production Fisher foundationality rows")?;
    let fisher_core = read_chunk_foundationality_score(fisher_store_db.as_ref(), "core:contract")?
        .context("production Fisher core foundationality row")?;
    let production_fisher_chunk_ids = vec!["core:contract".to_string(), low_chunk_id.clone()];
    let production_fisher_readback = consume_persisted_chunk_foundationality_fisher(
        production_fisher_store.as_ref(),
        &production_fisher_chunk_ids,
        &[2.0, 2.0],
        fisher_foundationality_report.computed_at_unix_ms as u64,
    )
    .context("consume persisted chunk foundationality Fisher multipliers")?;
    let production_fisher_reports =
        list_foundationality_weighted_fisher_reports(production_fisher_store.as_ref())
            .context("list production Fisher consumer reports")?;
    let production_fisher_ok = production_fisher_readback.readback_matches
        && production_fisher_reports == vec![production_fisher_readback.report.clone()]
        && (fisher_core.fisher_multiplier - 3.0).abs() < 1e-6
        && (production_fisher_readback.report.weighted_diagonal[0] - 6.0).abs() < 1e-6
        && production_fisher_readback.report.multiplier_source
            == FOUNDATIONALITY_FISHER_MULTIPLIER_SOURCE
        && production_fisher_readback.report.source_foundationality_cf
            == CF_MEJEPA_CHUNK_FOUNDATIONALITY;

    let lambda_fisher_ablation = synthetic_lambda_fisher_ablation(
        &lambda_fisher_ablation_db_path,
        &edges,
        &low_chunk_id,
        started_at_unix_ms,
    )
    .context("run synthetic lambda_fisher ablation")?;
    let lambda_fisher_ablation_ok = lambda_fisher_ablation.passes;
    let lambda_found_300x8_ablation =
        lambda_found_300x8_swebench_ablation(&lambda_found_ablation_db_path, started_at_unix_ms)
            .context("run 300x8 lambda_found ablation")?;
    let lambda_found_ablation_ok = lambda_found_300x8_ablation.passes;

    let python_sources = real_python_sources();
    let python_extraction =
        extract_python_chunk_dependency_edges(&python_sources).context("extract Python edges")?;
    let expected_edge_kinds = [
        "call",
        "data_flow",
        "import",
        "inheritance",
        "test_verifies",
        "type",
    ];
    let edge_kinds_ok = expected_edge_kinds.iter().all(|kind| {
        python_extraction
            .edge_kind_counts
            .get(*kind)
            .copied()
            .unwrap_or(0)
            > 0
    });
    ensure!(
        edge_kinds_ok,
        "Python extraction missed one or more required edge kinds"
    );
    let source_report = compute_chunk_foundationality(
        &python_extraction.edges,
        started_at_unix_ms,
        ChunkFoundationalityConfig::default(),
    )
    .context("compute Python-source foundationality")?;
    let source_db = open_infer_rocksdb(&source_db_path).context("open source-edge RocksDB")?;
    persist_chunk_foundationality_report_sync_readback(
        source_db.as_ref(),
        &python_extraction.edges,
        &source_report,
    )
    .context("persist Python-source foundationality report")?;
    let source_direct_edges = read_all_chunk_dependency_edges(source_db.as_ref())?;
    let source_direct_scores = read_all_chunk_foundationality_scores(source_db.as_ref())?;
    drop(source_db);
    let source_reopened = open_infer_rocksdb(&source_db_path).context("reopen source RocksDB")?;
    let source_reopened_edges = read_all_chunk_dependency_edges(source_reopened.as_ref())?;
    let source_reopened_scores = read_all_chunk_foundationality_scores(source_reopened.as_ref())?;
    let source_cf_readback_ok = source_direct_edges == source_reopened_edges
        && source_direct_scores == source_reopened_scores
        && source_reopened_edges.len() == python_extraction.edges.len()
        && source_reopened_scores.len() == source_report.scores.len();
    let source_core_foundational = source_report.scores.iter().any(|score| {
        score.chunk_id.starts_with("pkg/core.py::") && score.foundationality_score >= 0.5
    });
    let source_extractor_ok = edge_kinds_ok && source_cf_readback_ok && source_core_foundational;

    let disconnected_rejected = compute_chunk_foundationality(
        &[
            ChunkDependencyEdge::new("a", "b", "call", 1.0, "fixture:disc-a"),
            ChunkDependencyEdge::new("c", "d", "call", 1.0, "fixture:disc-b"),
        ],
        started_at_unix_ms,
        ChunkFoundationalityConfig::default(),
    )
    .unwrap_err()
    .to_string()
    .contains(MEJEPA_FOUNDATIONALITY_GRAPH_DISCONNECTED);

    let hierarchy_matches = report.scores[0].chunk_id == "core:contract"
        && report.scores[0].foundationality_score > report.scores[1].foundationality_score
        && report.scores[1].foundationality_score
            > report.scores.last().unwrap().foundationality_score;
    let cf_readback_ok = direct_edges.len() == edges.len()
        && direct_scores.len() == report.scores.len()
        && reopened_edges == direct_edges
        && reopened_scores == direct_scores
        && reopened_core == direct_core
        && edge_count_after_reopen == edges.len() as u64
        && score_count_after_reopen == report.scores.len() as u64;
    let bedrock_ok = high_report.bedrock_touched
        && high_report.top_touched_chunks[0].chunk_id == "core:contract"
        && !low_report.bedrock_touched;
    let sampler_ok = ablation
        .iter()
        .all(|case| (case.observed_bedrock_weight - case.expected_bedrock_weight).abs() < 1e-6);
    let fisher_ok = weighted_fisher > leaf_fisher && (weighted_fisher - 4.0).abs() < 1e-6;
    let compression_ok = core_compression < leaf_compression;
    let all_passed = hierarchy_matches
        && cf_readback_ok
        && bedrock_ok
        && sampler_ok
        && fisher_ok
        && production_fisher_ok
        && lambda_fisher_ablation_ok
        && lambda_found_ablation_ok
        && compression_ok
        && source_extractor_ok
        && disconnected_rejected;

    let evidence = json!({
        "task": "TASK-EK-016",
        "issue": 339,
        "passes": all_passed,
        "all_passed": all_passed,
        "sot": {
            "dependency_graph_cf": CF_MEJEPA_CHUNK_DEPENDENCY_GRAPH,
            "foundationality_cf": CF_MEJEPA_CHUNK_FOUNDATIONALITY,
            "db_path": db_path,
            "fsv_path": fsv_path,
            "canonical_path": canonical_path,
            "production_fisher_db_path": production_fisher_db_path,
            "lambda_fisher_ablation_db_path": lambda_fisher_ablation_db_path,
            "lambda_found_ablation_db_path": lambda_found_ablation_db_path,
            "inner_llm_invoked": false
        },
        "graph": {
            "dependency_graph_sha256": report.dependency_graph_sha256,
            "node_count": report.node_count,
            "edge_count": report.edge_count,
            "solver_pushes": report.solver_pushes,
            "solver_total_mass": report.solver_total_mass,
            "top_chunk": top,
            "lowest_chunk_id": low_chunk_id
        },
        "readback": {
            "edge_count_before": edge_count_before,
            "score_count_before": score_count_before,
            "direct_edges": direct_edges.len(),
            "direct_scores": direct_scores.len(),
            "reopened_edges": reopened_edges.len(),
            "reopened_scores": reopened_scores.len(),
            "core_direct_equals_reopen": reopened_core == direct_core,
            "cf_readback_ok": cf_readback_ok
        },
        "python_source_extractor": {
            "analyzer": python_extraction.analyzer,
            "source_count": python_extraction.source_count,
            "edge_count": python_extraction.edge_count,
            "edge_kind_counts": python_extraction.edge_kind_counts,
            "source_db_path": source_db_path,
            "source_dependency_graph_sha256": source_report.dependency_graph_sha256,
            "source_node_count": source_report.node_count,
            "source_score_count": source_report.scores.len(),
            "source_top_chunk": source_report.scores.first(),
            "source_core_foundational": source_core_foundational,
            "source_cf_readback_ok": source_cf_readback_ok,
            "source_extractor_ok": source_extractor_ok
        },
        "bedrock_consistency": {
            "high_report": high_report,
            "low_report": low_report,
            "bedrock_ok": bedrock_ok
        },
        "sampler_ablation": ablation,
        "lambda_found_300x8_ablation": lambda_found_300x8_ablation,
        "fisher": {
            "core_weighted": weighted_fisher,
            "leaf_weighted": leaf_fisher,
            "helper_fisher_ok": fisher_ok,
            "production_consumer": {
                "readback": production_fisher_readback,
                "persisted_report_count": production_fisher_reports.len(),
                "core_persisted_multiplier": fisher_core.fisher_multiplier,
                "production_fisher_ok": production_fisher_ok
            },
            "lambda_fisher_ablation": lambda_fisher_ablation
        },
        "compression": {
            "core_aggressiveness": core_compression,
            "leaf_aggressiveness": leaf_compression,
            "compression_ok": compression_ok
        },
        "edge_cases": {
            "disconnected_rejected": disconnected_rejected
        },
        "checks": {
            "hierarchy_matches": hierarchy_matches,
            "sampler_ok": sampler_ok,
            "lambda_found_ablation_ok": lambda_found_ablation_ok,
            "production_fisher_ok": production_fisher_ok,
            "lambda_fisher_ablation_ok": lambda_fisher_ablation_ok
        }
    });
    write_json_0600(&fsv_path, &evidence).context("write TASK-EK-016 run FSV JSON")?;
    write_json_0600(&canonical_path, &evidence).context("write TASK-EK-016 canonical FSV JSON")?;
    let fsv_sha256 = sha256_file(&fsv_path).context("hash TASK-EK-016 FSV")?;
    let stdout = json!({
        "all_passed": all_passed,
        "fsv_path": fsv_path,
        "canonical_path": canonical_path,
        "fsv_sha256": fsv_sha256,
        "dependency_graph_sha256": report.dependency_graph_sha256
    });
    println!("{}", serde_json::to_string_pretty(&stdout)?);
    ensure!(all_passed, "TASK-EK-016 FSV failed checks");
    Ok(())
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct SamplerAblationCase {
    lambda_found: f32,
    observed_leaf_weight: f32,
    observed_bedrock_weight: f32,
    expected_bedrock_weight: f32,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct LambdaFisherAblationReport {
    lambda_values: Vec<f32>,
    synthetic_task_a_theta_star: Vec<f32>,
    synthetic_task_b_target: Vec<f32>,
    ewc_lambda: f32,
    flat_baseline_core_forgetting: f32,
    flat_baseline_leaf_task_b_error: f32,
    recommended_lambda_fisher: f32,
    recommended_reason: String,
    cases: Vec<LambdaFisherAblationCase>,
    passes: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct LambdaFisherAblationCase {
    lambda_fisher: f32,
    core_persisted_multiplier: f32,
    leaf_persisted_multiplier: f32,
    weighted_fisher_diagonal: Vec<f32>,
    solved_theta_after_task_b: Vec<f32>,
    core_forgetting: f32,
    leaf_task_b_error: f32,
    retention_gain_vs_flat: f32,
    leaf_plasticity_regression_vs_flat: f32,
    consumer_readback_matches: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct LambdaFound300x8AblationReport {
    corpus_index: String,
    corpus_entries: usize,
    train_count: usize,
    calibration_count: usize,
    holdout_count: usize,
    patch_modifies_edge_count: usize,
    patch_graph_node_count: usize,
    weak_component_count: usize,
    score_count: usize,
    score_readback_ok: bool,
    foundationality_score_min: f32,
    foundationality_score_max: f32,
    foundationality_score_mean: f32,
    lambda_values: Vec<f32>,
    baseline_lambda_found: f32,
    recommended_lambda_found: f32,
    recommended_reason: String,
    cases: Vec<LambdaFound300x8Case>,
    passes: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct LambdaFound300x8Case {
    lambda_found: f32,
    selected_total: u64,
    selected_unique_examples: usize,
    selected_mean_foundationality_score: f32,
    global_holdout_correlation: Option<f32>,
    global_holdout_accuracy: f32,
    mean_defined_per_cell_correlation: Option<f32>,
    global_correlation_delta_vs_baseline: Option<f32>,
    global_accuracy_delta_vs_baseline: f32,
    mean_per_cell_correlation_delta_vs_baseline: Option<f32>,
    per_cell: BTreeMap<String, PerCellLambdaFoundMetric>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct PerCellLambdaFoundMetric {
    sample_count: usize,
    correlation: Option<f32>,
    accuracy: f32,
    correlation_delta_vs_baseline: Option<f32>,
    accuracy_delta_vs_baseline: f32,
}

#[derive(Debug, Clone)]
struct SwebenchPatchEntry {
    task_id: String,
    category_slug: String,
    bucket: String,
    patch_path: PathBuf,
    touched_chunk_ids: Vec<String>,
}

#[derive(Debug, Clone)]
struct PredictionRow {
    cell: String,
    probability: f32,
    actual: bool,
}

fn foundationality_sampler_ablation(bedrock_score: f32) -> Result<Vec<SamplerAblationCase>> {
    let mut cases = Vec::new();
    for lambda_found in [0.0_f32, 0.5, 1.0, 2.0] {
        let cfg = TrainingConfig {
            sampling_foundationality_lambda: lambda_found,
            sampling_force_threshold: 1.0,
            ..TrainingConfig::default()
        };
        let sampler = BatchSampler::new_with_foundationality_scores(
            cfg,
            vec![0.2, 0.2],
            vec![false, false],
            vec![0.0, 0.0],
            vec!["leaf".to_string(), "bedrock".to_string()],
            vec![0.0, bedrock_score],
            Vec::new(),
            PatchSimilarityGraph {
                neighbors: vec![Vec::new(); 2],
            },
        )
        .context("construct foundationality sampler")?;
        cases.push(SamplerAblationCase {
            lambda_found,
            observed_leaf_weight: sampler.weights[0],
            observed_bedrock_weight: sampler.weights[1],
            expected_bedrock_weight: 0.2 * (1.0 + lambda_found * bedrock_score),
        });
    }
    Ok(cases)
}

fn lambda_found_300x8_swebench_ablation(
    db_path: &Path,
    started_at_unix_ms: i64,
) -> Result<LambdaFound300x8AblationReport> {
    let entries = load_swebench_patch_entries(Path::new(SWE_300X8_INDEX_PATH))?;
    ensure!(
        entries.len() == 2400,
        "300x8 lambda_found ablation requires the verified 2400-entry corpus"
    );
    let edges = entries
        .iter()
        .flat_map(|entry| {
            entry.touched_chunk_ids.iter().map(move |chunk_id| {
                ChunkDependencyEdge::new(
                    format!("patch:{}:{}", entry.task_id, entry.category_slug),
                    chunk_id.clone(),
                    "patch_modifies",
                    1.0,
                    format!(
                        "swebench_300x8:{}:{}",
                        entry.bucket,
                        entry.patch_path.display()
                    ),
                )
            })
        })
        .collect::<Vec<_>>();
    let weak_components = weak_component_count(&edges);
    let report = compute_chunk_foundationality(
        &edges,
        started_at_unix_ms + 100,
        ChunkFoundationalityConfig {
            connected_required: false,
            ..ChunkFoundationalityConfig::default()
        },
    )
    .context("compute 300x8 patch-modifies foundationality")?;
    let db = open_infer_rocksdb(db_path).context("open lambda_found 300x8 RocksDB")?;
    persist_chunk_foundationality_report_sync_readback(db.as_ref(), &edges, &report)
        .context("persist 300x8 patch-modifies foundationality")?;
    let persisted_scores = read_all_chunk_foundationality_scores(db.as_ref())
        .context("read back 300x8 foundationality scores")?;
    let report_score_by_chunk = report
        .scores
        .iter()
        .map(|score| (score.chunk_id.clone(), score.clone()))
        .collect::<BTreeMap<_, _>>();
    let score_readback_ok = persisted_scores.len() == report.scores.len()
        && persisted_scores
            .iter()
            .all(|score| report_score_by_chunk.get(&score.chunk_id) == Some(score));
    let score_by_chunk = persisted_scores
        .iter()
        .map(|score| (score.chunk_id.clone(), score.foundationality_score))
        .collect::<BTreeMap<_, _>>();
    let mut score_by_entry = BTreeMap::new();
    for entry in &entries {
        let score = entry
            .touched_chunk_ids
            .iter()
            .filter_map(|chunk_id| score_by_chunk.get(chunk_id).copied())
            .fold(None, |best: Option<f32>, score| {
                Some(best.map_or(score, |current| current.max(score)))
            })
            .with_context(|| {
                format!(
                    "missing foundationality score for touched chunks in {}",
                    entry.patch_path.display()
                )
            })?;
        score_by_entry.insert(
            swebench_entry_key(&entry.bucket, &entry.task_id, &entry.category_slug),
            score,
        );
    }

    let mut dataset = load_corpus_index_dataset(Path::new(SWE_300X8_INDEX_PATH))
        .context("load 300x8 training dataset")?;
    assign_foundationality_scores(&mut dataset.train.examples, "train", &score_by_entry)?;
    assign_foundationality_scores(
        &mut dataset.calibration.examples,
        "calibration",
        &score_by_entry,
    )?;
    assign_foundationality_scores(&mut dataset.holdout.examples, "holdout", &score_by_entry)?;

    let all_example_scores = dataset
        .train
        .examples
        .iter()
        .chain(dataset.calibration.examples.iter())
        .chain(dataset.holdout.examples.iter())
        .map(|example| example.foundationality_score)
        .collect::<Vec<_>>();
    let foundationality_score_min = all_example_scores
        .iter()
        .copied()
        .fold(f32::INFINITY, f32::min);
    let foundationality_score_max = all_example_scores
        .iter()
        .copied()
        .fold(f32::NEG_INFINITY, f32::max);
    let foundationality_score_mean =
        all_example_scores.iter().sum::<f32>() / all_example_scores.len() as f32;

    let mut cases = Vec::new();
    for lambda_found in [0.0_f32, 0.5, 1.0, 2.0] {
        cases.push(run_lambda_found_300x8_case(&dataset, lambda_found)?);
    }
    apply_lambda_found_baseline_deltas(&mut cases)?;
    let (recommended_lambda_found, recommended_reason) =
        recommend_lambda_found(&cases).context("recommend lambda_found")?;

    let cases_ok = cases.len() == 4
        && cases.iter().all(|case| {
            case.selected_total > 0
                && case.global_holdout_correlation.is_some()
                && case.per_cell.len() == MutationCategory::ALL.len()
        });
    let passes = score_readback_ok
        && report.node_count > entries.len()
        && weak_components > 1
        && (foundationality_score_max - foundationality_score_min) > 1e-6
        && dataset.train.examples.len() == 1920
        && dataset.calibration.examples.len() == 240
        && dataset.holdout.examples.len() == 240
        && cases_ok;

    Ok(LambdaFound300x8AblationReport {
        corpus_index: SWE_300X8_INDEX_PATH.to_string(),
        corpus_entries: entries.len(),
        train_count: dataset.train.examples.len(),
        calibration_count: dataset.calibration.examples.len(),
        holdout_count: dataset.holdout.examples.len(),
        patch_modifies_edge_count: edges.len(),
        patch_graph_node_count: report.node_count,
        weak_component_count: weak_components,
        score_count: persisted_scores.len(),
        score_readback_ok,
        foundationality_score_min,
        foundationality_score_max,
        foundationality_score_mean,
        lambda_values: cases.iter().map(|case| case.lambda_found).collect(),
        baseline_lambda_found: 0.0,
        recommended_lambda_found,
        recommended_reason,
        cases,
        passes,
    })
}

fn load_swebench_patch_entries(index_path: &Path) -> Result<Vec<SwebenchPatchEntry>> {
    let root: serde_json::Value = serde_json::from_slice(
        &fs::read(index_path)
            .with_context(|| format!("read corpus index {}", index_path.display()))?,
    )?;
    let entries = root
        .get("entries")
        .and_then(|value| value.as_array())
        .context("corpus index missing entries array")?;
    let mut out = Vec::with_capacity(entries.len());
    for (idx, entry) in entries.iter().enumerate() {
        let field = |name: &str| -> Result<&str> {
            entry
                .get(name)
                .and_then(|value| value.as_str())
                .with_context(|| format!("corpus entry {idx} missing string field {name}"))
        };
        let task_id = field("task_id")?.to_string();
        let category_slug = field("category")?.to_string();
        let bucket = field("bucket")?.to_string();
        let repo = field("repo")?.to_string();
        let patch_rel = field("patch_path")?;
        let patch_path = Path::new(SWE_300X8_CORPUS_DIR).join(patch_rel);
        let patch_text = fs::read_to_string(&patch_path)
            .with_context(|| format!("read patch {}", patch_path.display()))?;
        let touched_chunk_ids = parse_touched_patch_files(&repo, &patch_text);
        ensure!(
            !touched_chunk_ids.is_empty(),
            "patch {} did not expose any touched files",
            patch_path.display()
        );
        out.push(SwebenchPatchEntry {
            task_id,
            category_slug,
            bucket,
            patch_path,
            touched_chunk_ids,
        });
    }
    Ok(out)
}

fn parse_touched_patch_files(repo: &str, patch_text: &str) -> Vec<String> {
    let mut files = BTreeSet::new();
    for line in patch_text.lines() {
        if let Some(rest) = line.strip_prefix("diff --git a/") {
            if let Some((left, _right)) = rest.split_once(" b/") {
                files.insert(chunk_id_for_patch_file(repo, left));
            }
        } else if let Some(path) = line.strip_prefix("+++ ") {
            if let Some(path) = normalize_diff_path(path) {
                files.insert(chunk_id_for_patch_file(repo, path));
            }
        } else if let Some(path) = line.strip_prefix("--- ") {
            if let Some(path) = normalize_diff_path(path) {
                files.insert(chunk_id_for_patch_file(repo, path));
            }
        }
    }
    files.into_iter().collect()
}

fn normalize_diff_path(path: &str) -> Option<&str> {
    if path == "/dev/null" {
        return None;
    }
    path.strip_prefix("a/")
        .or_else(|| path.strip_prefix("b/"))
        .or(Some(path))
}

fn chunk_id_for_patch_file(repo: &str, path: &str) -> String {
    format!("file:{repo}:{path}")
}

fn swebench_entry_key(bucket: &str, task_id: &str, category_slug: &str) -> String {
    format!("{bucket}\t{task_id}\t{category_slug}")
}

fn assign_foundationality_scores(
    examples: &mut [HoldoutExample],
    bucket: &str,
    score_by_entry: &BTreeMap<String, f32>,
) -> Result<()> {
    for example in examples {
        let key = swebench_entry_key(bucket, &example.task_id, category_slug(example.category));
        example.foundationality_score = *score_by_entry
            .get(&key)
            .with_context(|| format!("missing 300x8 foundationality score for {key}"))?;
    }
    Ok(())
}

fn run_lambda_found_300x8_case(
    dataset: &context_graph_mejepa_train::trainer::TrainingDataset,
    lambda_found: f32,
) -> Result<LambdaFound300x8Case> {
    let train = &dataset.train.examples;
    let mut sampler = BatchSampler::new_with_foundationality_scores(
        TrainingConfig {
            epochs: 1,
            batch_size: 64,
            random_seed: 0x2026_0518,
            cross_task_transfer_probability: 0.0,
            adversarial_mix_ratio: 0.0,
            sampling_foundationality_lambda: lambda_found,
            ..TrainingConfig::default()
        },
        (0..train.len())
            .map(|i| 0.1 + (i % 10) as f32 * 0.05)
            .collect(),
        (0..train.len()).map(|i| i % 17 == 0).collect(),
        vec![0.0; train.len()],
        train.iter().map(|ex| ex.task_id.clone()).collect(),
        train.iter().map(|ex| ex.foundationality_score).collect(),
        Vec::new(),
        PatchSimilarityGraph {
            neighbors: vec![Vec::new(); train.len()],
        },
    )
    .context("construct 300x8 lambda_found sampler")?;
    let mut selected_counts = vec![0_u64; train.len()];
    let batches = train.len().div_ceil(sampler.config.batch_size);
    for batch_idx in 0..batches {
        let plan = sampler
            .next_batch_with_cross_task(batch_idx % train.len(), sampler.config.batch_size)?;
        for idx in plan.indices {
            selected_counts[idx] += 1;
        }
    }
    let selected_total = selected_counts.iter().sum::<u64>();
    let selected_unique_examples = selected_counts.iter().filter(|count| **count > 0).count();
    let selected_mean_foundationality_score = train
        .iter()
        .zip(&selected_counts)
        .map(|(example, count)| example.foundationality_score * *count as f32)
        .sum::<f32>()
        / selected_total.max(1) as f32;
    let predictions = weighted_knn_predictions(train, &selected_counts, &dataset.holdout.examples)?;
    let (global_holdout_correlation, global_holdout_accuracy, mut per_cell) =
        evaluate_lambda_found_predictions(&predictions);
    Ok(LambdaFound300x8Case {
        lambda_found,
        selected_total,
        selected_unique_examples,
        selected_mean_foundationality_score,
        global_holdout_correlation,
        global_holdout_accuracy,
        mean_defined_per_cell_correlation: mean_defined_per_cell_correlation(&per_cell),
        global_correlation_delta_vs_baseline: None,
        global_accuracy_delta_vs_baseline: 0.0,
        mean_per_cell_correlation_delta_vs_baseline: None,
        per_cell: {
            for metric in per_cell.values_mut() {
                metric.correlation_delta_vs_baseline = None;
                metric.accuracy_delta_vs_baseline = 0.0;
            }
            per_cell
        },
    })
}

fn weighted_knn_predictions(
    train: &[HoldoutExample],
    selected_counts: &[u64],
    holdout: &[HoldoutExample],
) -> Result<Vec<PredictionRow>> {
    ensure!(
        train.len() == selected_counts.len(),
        "selected-count vector length must match train split"
    );
    let train_hashes = train
        .iter()
        .map(|example| task_hash_unit(&example.task_id))
        .collect::<Vec<_>>();
    let mut rows = Vec::with_capacity(holdout.len());
    for holdout_example in holdout {
        let holdout_hash = task_hash_unit(&holdout_example.task_id);
        let mut numerator = 0.0_f32;
        let mut denominator = 0.0_f32;
        for ((train_example, selected_count), train_hash) in
            train.iter().zip(selected_counts).zip(&train_hashes)
        {
            if *selected_count == 0 {
                continue;
            }
            let category_penalty = if train_example.category == holdout_example.category {
                0.0
            } else {
                0.75
            };
            let language_penalty = if train_example.language == holdout_example.language {
                0.0
            } else {
                0.5
            };
            let similarity = 1.0
                / (0.05
                    + (train_hash - holdout_hash).abs()
                    + (train_example.foundationality_score
                        - holdout_example.foundationality_score)
                        .abs()
                        * 0.25
                    + category_penalty
                    + language_penalty);
            let weight = *selected_count as f32 * similarity;
            denominator += weight;
            if train_example.actual_oracle_pass {
                numerator += weight;
            }
        }
        ensure!(
            denominator > 0.0,
            "weighted KNN denominator must be positive for {}",
            holdout_example.task_id
        );
        rows.push(PredictionRow {
            cell: cell_key(holdout_example.category, holdout_example.language),
            probability: (numerator / denominator).clamp(0.0, 1.0),
            actual: holdout_example.actual_oracle_pass,
        });
    }
    Ok(rows)
}

fn evaluate_lambda_found_predictions(
    rows: &[PredictionRow],
) -> (Option<f32>, f32, BTreeMap<String, PerCellLambdaFoundMetric>) {
    let global_correlation = pearson_probability_label(
        rows.iter().map(|row| row.probability),
        rows.iter().map(|row| row.actual),
    );
    let global_accuracy = accuracy(rows);
    let mut by_cell = BTreeMap::<String, Vec<&PredictionRow>>::new();
    for row in rows {
        by_cell.entry(row.cell.clone()).or_default().push(row);
    }
    let mut per_cell = BTreeMap::new();
    for category in MutationCategory::ALL {
        let key = cell_key(category, Lang::Python);
        let cell_rows = by_cell.remove(&key).unwrap_or_default();
        let correlation = pearson_probability_label(
            cell_rows.iter().map(|row| row.probability),
            cell_rows.iter().map(|row| row.actual),
        );
        let accuracy = accuracy_refs(&cell_rows);
        per_cell.insert(
            key,
            PerCellLambdaFoundMetric {
                sample_count: cell_rows.len(),
                correlation,
                accuracy,
                correlation_delta_vs_baseline: None,
                accuracy_delta_vs_baseline: 0.0,
            },
        );
    }
    (global_correlation, global_accuracy, per_cell)
}

fn apply_lambda_found_baseline_deltas(cases: &mut [LambdaFound300x8Case]) -> Result<()> {
    let baseline = cases
        .iter()
        .find(|case| (case.lambda_found - 0.0).abs() < f32::EPSILON)
        .context("missing lambda_found=0 baseline")?
        .clone();
    for case in cases {
        case.global_correlation_delta_vs_baseline = match (
            case.global_holdout_correlation,
            baseline.global_holdout_correlation,
        ) {
            (Some(current), Some(base)) => Some(current - base),
            _ => None,
        };
        case.global_accuracy_delta_vs_baseline =
            case.global_holdout_accuracy - baseline.global_holdout_accuracy;
        case.mean_per_cell_correlation_delta_vs_baseline = match (
            case.mean_defined_per_cell_correlation,
            baseline.mean_defined_per_cell_correlation,
        ) {
            (Some(current), Some(base)) => Some(current - base),
            _ => None,
        };
        for (key, metric) in &mut case.per_cell {
            let base = baseline
                .per_cell
                .get(key)
                .with_context(|| format!("baseline missing per-cell metric for {key}"))?;
            metric.correlation_delta_vs_baseline = match (metric.correlation, base.correlation) {
                (Some(current), Some(base)) => Some(current - base),
                _ => None,
            };
            metric.accuracy_delta_vs_baseline = metric.accuracy - base.accuracy;
        }
    }
    Ok(())
}

fn recommend_lambda_found(cases: &[LambdaFound300x8Case]) -> Result<(f32, String)> {
    let baseline = cases
        .iter()
        .find(|case| (case.lambda_found - 0.0).abs() < f32::EPSILON)
        .context("missing lambda_found=0 baseline")?;
    let best = cases
        .iter()
        .filter(|case| case.global_accuracy_delta_vs_baseline >= -0.005)
        .max_by(|left, right| {
            let left_score = left
                .mean_defined_per_cell_correlation
                .or(left.global_holdout_correlation)
                .unwrap_or(f32::NEG_INFINITY);
            let right_score = right
                .mean_defined_per_cell_correlation
                .or(right.global_holdout_correlation)
                .unwrap_or(f32::NEG_INFINITY);
            left_score
                .partial_cmp(&right_score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| {
                    left.global_holdout_accuracy
                        .partial_cmp(&right.global_holdout_accuracy)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
        })
        .context("no lambda_found candidate survived accuracy guard")?;
    let best_delta = best
        .mean_per_cell_correlation_delta_vs_baseline
        .or(best.global_correlation_delta_vs_baseline)
        .unwrap_or(0.0);
    if best.lambda_found == baseline.lambda_found || best_delta.abs() < 0.001 {
        Ok((
            baseline.lambda_found,
            "300x8 patch-modifies ablation was null within 0.001 correlation; keep flat sampler lambda_found=0 until a stronger signal clears promotion guard".to_string(),
        ))
    } else {
        Ok((
            best.lambda_found,
            "best mean defined per-cell holdout correlation with global accuracy regression guard >= -0.005".to_string(),
        ))
    }
}

fn mean_defined_per_cell_correlation(
    per_cell: &BTreeMap<String, PerCellLambdaFoundMetric>,
) -> Option<f32> {
    let values = per_cell
        .values()
        .filter_map(|metric| metric.correlation)
        .collect::<Vec<_>>();
    if values.is_empty() {
        None
    } else {
        Some(values.iter().sum::<f32>() / values.len() as f32)
    }
}

fn pearson_probability_label(
    probabilities: impl Iterator<Item = f32>,
    actuals: impl Iterator<Item = bool>,
) -> Option<f32> {
    let xs = probabilities.collect::<Vec<_>>();
    let ys = actuals
        .map(|actual| if actual { 1.0_f32 } else { 0.0 })
        .collect::<Vec<_>>();
    if xs.len() != ys.len() || xs.len() < 2 {
        return None;
    }
    let x_mean = xs.iter().sum::<f32>() / xs.len() as f32;
    let y_mean = ys.iter().sum::<f32>() / ys.len() as f32;
    let mut numerator = 0.0_f32;
    let mut x_var = 0.0_f32;
    let mut y_var = 0.0_f32;
    for (x, y) in xs.iter().zip(&ys) {
        let dx = *x - x_mean;
        let dy = *y - y_mean;
        numerator += dx * dy;
        x_var += dx * dx;
        y_var += dy * dy;
    }
    let denom = (x_var * y_var).sqrt();
    if denom <= 1e-8 {
        None
    } else {
        Some((numerator / denom).clamp(-1.0, 1.0))
    }
}

fn accuracy(rows: &[PredictionRow]) -> f32 {
    if rows.is_empty() {
        return 0.0;
    }
    rows.iter()
        .filter(|row| (row.probability >= 0.5) == row.actual)
        .count() as f32
        / rows.len() as f32
}

fn accuracy_refs(rows: &[&PredictionRow]) -> f32 {
    if rows.is_empty() {
        return 0.0;
    }
    rows.iter()
        .filter(|row| (row.probability >= 0.5) == row.actual)
        .count() as f32
        / rows.len() as f32
}

fn weak_component_count(edges: &[ChunkDependencyEdge]) -> usize {
    let mut adjacency = BTreeMap::<String, BTreeSet<String>>::new();
    for edge in edges {
        adjacency
            .entry(edge.from_chunk_id.clone())
            .or_default()
            .insert(edge.to_chunk_id.clone());
        adjacency
            .entry(edge.to_chunk_id.clone())
            .or_default()
            .insert(edge.from_chunk_id.clone());
    }
    let mut visited = BTreeSet::new();
    let mut components = 0;
    for node in adjacency.keys() {
        if visited.contains(node) {
            continue;
        }
        components += 1;
        let mut stack = vec![node.clone()];
        while let Some(current) = stack.pop() {
            if !visited.insert(current.clone()) {
                continue;
            }
            if let Some(neighbors) = adjacency.get(&current) {
                for neighbor in neighbors {
                    if !visited.contains(neighbor) {
                        stack.push(neighbor.clone());
                    }
                }
            }
        }
    }
    components
}

fn task_hash_unit(task_id: &str) -> f32 {
    let digest = Sha256::digest(task_id.as_bytes());
    u32::from_be_bytes([digest[0], digest[1], digest[2], digest[3]]) as f32 / u32::MAX as f32
}

fn cell_key(category: MutationCategory, language: Lang) -> String {
    format!("{}::{}", category_slug(category), lang_slug(language))
}

fn category_slug(category: MutationCategory) -> &'static str {
    match category {
        MutationCategory::KnownGood => "known_good",
        MutationCategory::SubtleFlip => "subtle_flip",
        MutationCategory::OffByOne => "off_by_one",
        MutationCategory::SwapVariable => "swap_variable",
        MutationCategory::DeleteTestCall => "delete_test_call",
        MutationCategory::WrongFile => "wrong_file",
        MutationCategory::OverEngineer => "over_engineer",
        MutationCategory::CompileError => "compile_error",
    }
}

fn lang_slug(language: Lang) -> &'static str {
    match language {
        Lang::Rust => "rust",
        Lang::Python => "python",
        Lang::JavaScript => "javascript",
        Lang::TypeScript => "typescript",
        Lang::Go => "go",
        Lang::Java => "java",
        Lang::C => "c",
        Lang::Cpp => "cpp",
        Lang::CSharp => "csharp",
        Lang::Ruby => "ruby",
        Lang::Php => "php",
    }
}

fn synthetic_lambda_fisher_ablation(
    db_path: &Path,
    edges: &[ChunkDependencyEdge],
    leaf_chunk_id: &str,
    started_at_unix_ms: i64,
) -> Result<LambdaFisherAblationReport> {
    let task_a_theta_star = vec![1.0_f32, 1.0_f32];
    let task_b_target = vec![0.0_f32, 0.0_f32];
    let base_fisher = vec![1.0_f32, 1.0_f32];
    let ewc_lambda = 1.0_f32;
    let flat_theta =
        solve_quadratic_ewc(&task_a_theta_star, &task_b_target, &base_fisher, ewc_lambda);
    let flat_baseline_core_forgetting = (flat_theta[0] - task_a_theta_star[0]).powi(2);
    let flat_baseline_leaf_task_b_error = (flat_theta[1] - task_b_target[1]).powi(2);

    let storage = HealRocksStore::open(db_path).context("open lambda_fisher ablation RocksDB")?;
    let chunk_ids = vec!["core:contract".to_string(), leaf_chunk_id.to_string()];
    let mut cases = Vec::new();
    for (idx, lambda_fisher) in [0.0_f32, 0.5, 1.0, 2.0].into_iter().enumerate() {
        let report = compute_chunk_foundationality(
            edges,
            started_at_unix_ms + idx as i64 + 10,
            ChunkFoundationalityConfig {
                lambda_fisher,
                ..ChunkFoundationalityConfig::default()
            },
        )
        .with_context(|| format!("compute lambda_fisher={lambda_fisher} foundationality"))?;
        let db = storage.db();
        persist_chunk_foundationality_report_sync_readback(db.as_ref(), edges, &report)
            .with_context(|| format!("persist lambda_fisher={lambda_fisher} rows"))?;
        let readback = consume_persisted_chunk_foundationality_fisher(
            storage.as_ref(),
            &chunk_ids,
            &base_fisher,
            report.computed_at_unix_ms as u64,
        )
        .with_context(|| format!("consume lambda_fisher={lambda_fisher} Fisher multipliers"))?;
        let weighted_fisher = readback.report.weighted_diagonal.clone();
        let solved_theta = solve_quadratic_ewc(
            &task_a_theta_star,
            &task_b_target,
            &weighted_fisher,
            ewc_lambda,
        );
        let core_forgetting = (solved_theta[0] - task_a_theta_star[0]).powi(2);
        let leaf_task_b_error = (solved_theta[1] - task_b_target[1]).powi(2);
        let retention_gain_vs_flat =
            (flat_baseline_core_forgetting - core_forgetting) / flat_baseline_core_forgetting;
        let leaf_plasticity_regression_vs_flat =
            leaf_task_b_error - flat_baseline_leaf_task_b_error;
        cases.push(LambdaFisherAblationCase {
            lambda_fisher,
            core_persisted_multiplier: readback.report.entries[0].fisher_multiplier,
            leaf_persisted_multiplier: readback.report.entries[1].fisher_multiplier,
            weighted_fisher_diagonal: weighted_fisher,
            solved_theta_after_task_b: solved_theta,
            core_forgetting,
            leaf_task_b_error,
            retention_gain_vs_flat,
            leaf_plasticity_regression_vs_flat,
            consumer_readback_matches: readback.readback_matches,
        });
    }
    let recommended = cases
        .iter()
        .filter(|case| case.leaf_plasticity_regression_vs_flat <= 0.05)
        .max_by(|left, right| {
            left.retention_gain_vs_flat
                .partial_cmp(&right.retention_gain_vs_flat)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .context("no bounded-plasticity lambda_fisher candidate")?;
    let passes = cases.iter().all(|case| case.consumer_readback_matches)
        && recommended.lambda_fisher > 0.0
        && recommended.retention_gain_vs_flat >= 0.5
        && recommended.leaf_plasticity_regression_vs_flat <= 0.05;
    Ok(LambdaFisherAblationReport {
        lambda_values: cases.iter().map(|case| case.lambda_fisher).collect(),
        synthetic_task_a_theta_star: task_a_theta_star,
        synthetic_task_b_target: task_b_target,
        ewc_lambda,
        flat_baseline_core_forgetting,
        flat_baseline_leaf_task_b_error,
        recommended_lambda_fisher: recommended.lambda_fisher,
        recommended_reason:
            "max retention gain among candidates with leaf plasticity regression <= 0.05"
                .to_string(),
        cases,
        passes,
    })
}

fn solve_quadratic_ewc(
    theta_star: &[f32],
    target: &[f32],
    fisher: &[f32],
    ewc_lambda: f32,
) -> Vec<f32> {
    theta_star
        .iter()
        .zip(target)
        .zip(fisher)
        .map(|((star, target), fisher)| {
            (target + ewc_lambda * fisher * star) / (1.0 + ewc_lambda * fisher)
        })
        .collect()
}

fn fixture_edges() -> Vec<ChunkDependencyEdge> {
    vec![
        ChunkDependencyEdge::new("api:handler", "core:contract", "call", 1.0, "fixture"),
        ChunkDependencyEdge::new("api:handler", "core:types", "type", 0.7, "fixture"),
        ChunkDependencyEdge::new("worker:sync", "core:contract", "call", 1.0, "fixture"),
        ChunkDependencyEdge::new("worker:sync", "core:types", "type", 0.7, "fixture"),
        ChunkDependencyEdge::new("leaf:test_a", "api:handler", "test", 1.0, "fixture"),
        ChunkDependencyEdge::new("leaf:test_b", "worker:sync", "test", 1.0, "fixture"),
        ChunkDependencyEdge::new("core:types", "core:contract", "type", 0.5, "fixture"),
    ]
}

fn real_python_sources() -> Vec<PythonChunkSource> {
    vec![
        PythonChunkSource {
            chunk_id: "pkg/core.py".to_string(),
            module: "pkg.core".to_string(),
            path: "pkg/core.py".to_string(),
            source: r#"
BASE_LIMIT = 10

class Base:
    def validate(self, value: int) -> int:
        return value

def normalize(value: int) -> int:
    return value + BASE_LIMIT
"#
            .to_string(),
        },
        PythonChunkSource {
            chunk_id: "pkg/service.py".to_string(),
            module: "pkg.service".to_string(),
            path: "pkg/service.py".to_string(),
            source: r#"
from .core import BASE_LIMIT, Base, normalize

class Service(Base):
    def score(self, raw: Base) -> int:
        adjusted = normalize(raw.validate(BASE_LIMIT))
        return adjusted + BASE_LIMIT
"#
            .to_string(),
        },
        PythonChunkSource {
            chunk_id: "pkg/api.py".to_string(),
            module: "pkg.api".to_string(),
            path: "pkg/api.py".to_string(),
            source: r#"
from .service import Service

def handle(value: int) -> int:
    service = Service()
    return service.score(value)
"#
            .to_string(),
        },
        PythonChunkSource {
            chunk_id: "tests/test_service.py".to_string(),
            module: "tests.test_service".to_string(),
            path: "tests/test_service.py".to_string(),
            source: r#"
from pkg.service import Service

def test_score() -> None:
    assert Service().score(2) == 12
"#
            .to_string(),
        },
    ]
}

fn sha256_file(path: &Path) -> Result<String> {
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    Ok(hex::encode(Sha256::digest(&bytes)))
}
