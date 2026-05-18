use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{ensure, Context, Result};
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
use context_graph_mejepa_train::config::TrainingConfig;
use context_graph_mejepa_train::sampler::{BatchSampler, PatchSimilarityGraph};
use serde_json::json;
use sha2::{Digest, Sha256};

const FSV_ROOT: &str = "/mnt/d/contextgraph/fsv/task-ek-016-foundationality-fsv";

fn main() -> Result<()> {
    let started_at_unix_ms = current_unix_ms();
    let run_root = PathBuf::from(FSV_ROOT).join(format!("run-{started_at_unix_ms}"));
    let db_path = run_root.join("infer-rocksdb");
    let source_db_path = run_root.join("python-source-rocksdb");
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
        "fisher": {
            "core_weighted": weighted_fisher,
            "leaf_weighted": leaf_fisher,
            "fisher_ok": fisher_ok
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
            "sampler_ok": sampler_ok
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
