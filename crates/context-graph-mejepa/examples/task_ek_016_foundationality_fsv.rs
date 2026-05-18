use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{ensure, Context, Result};
use context_graph_mejepa::{
    apply_foundationality_fisher_multiplier, bedrock_consistency_for_chunks,
    compression_aggressiveness_from_foundationality, compute_chunk_foundationality, count_cf,
    current_unix_ms, open_infer_rocksdb, persist_chunk_foundationality_report_sync_readback,
    read_all_chunk_dependency_edges, read_all_chunk_foundationality_scores,
    read_chunk_foundationality_score, write_json_0600, ChunkDependencyEdge,
    ChunkFoundationalityConfig, MEJEPA_FOUNDATIONALITY_GRAPH_DISCONNECTED,
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

fn sha256_file(path: &Path) -> Result<String> {
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    Ok(hex::encode(Sha256::digest(&bytes)))
}
