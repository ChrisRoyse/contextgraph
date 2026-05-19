use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{ensure, Context, Result};
use context_graph_mejepa::{
    compute_chunk_foundationality, compute_library_foundationality, count_cf, current_unix_ms,
    open_infer_rocksdb, persist_library_foundationality_report_sync_readback,
    read_all_chunk_dependency_edges, read_all_cross_library_reference_counts,
    read_all_library_foundationality_scores, read_library_foundationality_report,
    read_library_registration, write_json_0600, ChunkDependencyEdge, ChunkFoundationalityConfig,
    LibraryId, LibraryRegistration,
};
use context_graph_mejepa_cf::{
    CF_MEJEPA_CHUNK_DEPENDENCY_GRAPH, CF_MEJEPA_CROSS_LIBRARY_REFERENCES,
    CF_MEJEPA_LIBRARY_FOUNDATIONALITY, CF_MEJEPA_LIBRARY_REGISTRY,
};
use serde_json::json;
use sha2::{Digest, Sha256};

const FSV_ROOT: &str = "/zfs/archive/contextgraph/fsv/task-ek-017-library-foundationality-fsv";

fn main() -> Result<()> {
    let started_at_unix_ms = current_unix_ms();
    let run_root = PathBuf::from(FSV_ROOT).join(format!("run-{started_at_unix_ms}"));
    let db_path = run_root.join("infer-rocksdb");
    let fsv_path = run_root.join("library_foundationality_fsv.json");
    let canonical_path = PathBuf::from(FSV_ROOT).join("library_foundationality_fsv.json");
    fs::create_dir_all(&run_root).context("create TASK-EK-017 FSV dir")?;

    let py = LibraryId::PythonSweBenchLite;
    let shakespeare = LibraryId::ShakespeareCanon;
    let custom = LibraryId::Custom("operator-demo".to_string());
    let config = ChunkFoundationalityConfig::default();

    let two_library_edges = two_library_fixture_edges();
    let two_library_registrations = vec![
        registration(
            py.clone(),
            "Python corpus",
            started_at_unix_ms,
            "fsv:python",
        ),
        registration(
            shakespeare.clone(),
            "Shakespeare canon",
            started_at_unix_ms,
            "fsv:shakespeare",
        ),
    ];
    let two_library_report = compute_library_foundationality(
        &two_library_edges,
        &two_library_registrations,
        started_at_unix_ms,
        config,
    )
    .context("compute two-library foundationality")?;
    let engineered_bedrock = two_library_report
        .cross_library_bedrock
        .first()
        .context("two-library cross-library bedrock")?;
    let engineered_bedrock_ok = engineered_bedrock.chunk_id == "py:core"
        && engineered_bedrock.library_id == py
        && engineered_bedrock.cross_library_upstream_count >= 2;

    let single_library_edges = single_library_fixture_edges();
    let single_registration = vec![registration(
        py.clone(),
        "Python corpus",
        started_at_unix_ms,
        "fsv:single-python",
    )];
    let single_chunk_report =
        compute_chunk_foundationality(&single_library_edges, started_at_unix_ms, config)
            .context("compute baseline single-corpus foundationality")?;
    let single_library_report = compute_library_foundationality(
        &single_library_edges,
        &single_registration,
        started_at_unix_ms,
        config,
    )
    .context("compute single-library foundationality")?;
    let single_chunk_scores = single_chunk_report
        .scores
        .iter()
        .map(|score| (score.chunk_id.clone(), score.foundationality_score))
        .collect::<BTreeMap<_, _>>();
    let single_reduces_to_chunk_pagerank = single_library_report.scores.iter().all(|score| {
        single_chunk_scores
            .get(&score.chunk_id)
            .map(|expected| {
                (score.foundationality_score_within_library - expected).abs() < 1e-6
                    && (score.foundationality_score_cross_library - expected).abs() < 1e-6
            })
            .unwrap_or(false)
    });

    let custom_edges = three_library_fixture_edges();
    let custom_registrations = vec![
        registration(
            py.clone(),
            "Python corpus",
            started_at_unix_ms,
            "fsv:python",
        ),
        registration(
            shakespeare.clone(),
            "Shakespeare canon",
            started_at_unix_ms,
            "fsv:shakespeare",
        ),
        registration(
            custom.clone(),
            "Operator-defined demo library",
            started_at_unix_ms,
            "fsv:operator-demo",
        ),
    ];
    let custom_report = compute_library_foundationality(
        &custom_edges,
        &custom_registrations,
        started_at_unix_ms,
        config,
    )
    .context("compute custom-library foundationality")?;
    let custom_registration_present = custom_report
        .registrations
        .iter()
        .any(|registration| registration.library_id == custom);
    let custom_scores_present = custom_report
        .scores
        .iter()
        .filter(|score| score.library_id == custom)
        .count()
        >= 3;
    let custom_cross_reference_present = custom_report
        .cross_library_references
        .iter()
        .any(|reference| reference.from_library_id == custom && reference.to_library_id == py);

    let db = open_infer_rocksdb(&db_path).context("open TASK-EK-017 DB")?;
    let edge_count_before = count_cf(db.as_ref(), CF_MEJEPA_CHUNK_DEPENDENCY_GRAPH)?;
    let library_count_before = count_cf(db.as_ref(), CF_MEJEPA_LIBRARY_REGISTRY)?;
    let score_count_before = count_cf(db.as_ref(), CF_MEJEPA_LIBRARY_FOUNDATIONALITY)?;
    let ref_count_before = count_cf(db.as_ref(), CF_MEJEPA_CROSS_LIBRARY_REFERENCES)?;
    persist_library_foundationality_report_sync_readback(
        db.as_ref(),
        &custom_edges,
        &custom_report,
    )
    .context("persist library foundationality report")?;
    let direct_edges = read_all_chunk_dependency_edges(db.as_ref())?;
    let direct_scores = read_all_library_foundationality_scores(db.as_ref())?;
    let direct_refs = read_all_cross_library_reference_counts(db.as_ref())?;
    let direct_custom_registration =
        read_library_registration(db.as_ref(), &custom)?.context("direct custom registration")?;
    let direct_query = read_library_foundationality_report(db.as_ref(), Some(&custom), 5)
        .context("direct custom query report")?;
    drop(db);

    let reopened = open_infer_rocksdb(&db_path).context("reopen TASK-EK-017 DB")?;
    let reopened_edges = read_all_chunk_dependency_edges(reopened.as_ref())?;
    let reopened_scores = read_all_library_foundationality_scores(reopened.as_ref())?;
    let reopened_refs = read_all_cross_library_reference_counts(reopened.as_ref())?;
    let reopened_custom_registration = read_library_registration(reopened.as_ref(), &custom)?
        .context("reopened custom registration")?;
    let reopened_query = read_library_foundationality_report(reopened.as_ref(), None, 5)
        .context("reopened all-library query report")?;
    let edge_count_after = count_cf(reopened.as_ref(), CF_MEJEPA_CHUNK_DEPENDENCY_GRAPH)?;
    let library_count_after = count_cf(reopened.as_ref(), CF_MEJEPA_LIBRARY_REGISTRY)?;
    let score_count_after = count_cf(reopened.as_ref(), CF_MEJEPA_LIBRARY_FOUNDATIONALITY)?;
    let ref_count_after = count_cf(reopened.as_ref(), CF_MEJEPA_CROSS_LIBRARY_REFERENCES)?;

    let edge_library_ids_preserved = reopened_edges.iter().any(|edge| {
        edge.from_library_id == custom && edge.to_library_id == py && edge.to_chunk_id == "py:core"
    }) && direct_edges == reopened_edges;
    let custom_registration_roundtrip = direct_custom_registration == reopened_custom_registration;
    let score_readback_ok = direct_scores == reopened_scores
        && reopened_scores.len() == custom_report.scores.len()
        && score_count_after == custom_report.scores.len() as u64;
    let cross_reference_readback_ok = direct_refs == reopened_refs
        && reopened_refs.len() == custom_report.cross_library_references.len()
        && ref_count_after == custom_report.cross_library_references.len() as u64;
    let query_surfaces_ok = !direct_query.library_bedrock.is_empty()
        && reopened_query.cross_library_bedrock[0].chunk_id == "py:core";
    let custom_registration_ok = custom_registration_present
        && custom_scores_present
        && custom_cross_reference_present
        && custom_registration_roundtrip;
    let cf_readback_ok = edge_library_ids_preserved
        && score_readback_ok
        && cross_reference_readback_ok
        && library_count_after == custom_report.registrations.len() as u64
        && edge_count_after == custom_edges.len() as u64;
    let all_passed = engineered_bedrock_ok
        && single_reduces_to_chunk_pagerank
        && custom_registration_ok
        && cf_readback_ok
        && query_surfaces_ok;

    let evidence = json!({
        "task": "TASK-EK-017",
        "issue": 340,
        "passes": all_passed,
        "all_passed": all_passed,
        "sot": {
            "dependency_graph_cf": CF_MEJEPA_CHUNK_DEPENDENCY_GRAPH,
            "library_registry_cf": CF_MEJEPA_LIBRARY_REGISTRY,
            "library_foundationality_cf": CF_MEJEPA_LIBRARY_FOUNDATIONALITY,
            "cross_library_references_cf": CF_MEJEPA_CROSS_LIBRARY_REFERENCES,
            "db_path": db_path,
            "fsv_path": fsv_path,
            "canonical_path": canonical_path,
            "inner_llm_invoked": false
        },
        "sot_cases": {
            "two_synthetic_libraries_identify_engineered_bedrock": {
                "passed": engineered_bedrock_ok,
                "top_cross_library_bedrock": engineered_bedrock
            },
            "single_library_reduces_to_task_ek_016": {
                "passed": single_reduces_to_chunk_pagerank,
                "baseline_graph_sha256": single_chunk_report.dependency_graph_sha256,
                "library_graph_sha256": single_library_report.cross_library_graph_sha256
            },
            "custom_library_registration_contributes": {
                "passed": custom_registration_ok,
                "custom_library_id": custom.slug(),
                "custom_scores_present": custom_scores_present,
                "custom_cross_reference_present": custom_cross_reference_present,
                "custom_registration_roundtrip": custom_registration_roundtrip
            },
            "reopen_cf_readback_preserves_library_id": {
                "passed": cf_readback_ok,
                "edge_library_ids_preserved": edge_library_ids_preserved,
                "score_readback_ok": score_readback_ok,
                "cross_reference_readback_ok": cross_reference_readback_ok
            }
        },
        "readback": {
            "edge_count_before": edge_count_before,
            "library_count_before": library_count_before,
            "score_count_before": score_count_before,
            "ref_count_before": ref_count_before,
            "edge_count_after": edge_count_after,
            "library_count_after": library_count_after,
            "score_count_after": score_count_after,
            "ref_count_after": ref_count_after,
            "direct_edges": direct_edges.len(),
            "reopened_edges": reopened_edges.len(),
            "direct_scores": direct_scores.len(),
            "reopened_scores": reopened_scores.len(),
            "direct_refs": direct_refs.len(),
            "reopened_refs": reopened_refs.len()
        },
        "query_surfaces": {
            "passed": query_surfaces_ok,
            "cli_tool": "context-graph-cli mejepa library-foundationality",
            "mcp_tool": "mcp__cgreality__mejepa_library_foundationality",
            "direct_custom_query": direct_query,
            "reopened_all_query": reopened_query
        },
        "checks": {
            "engineered_bedrock_ok": engineered_bedrock_ok,
            "single_reduces_to_chunk_pagerank": single_reduces_to_chunk_pagerank,
            "custom_registration_ok": custom_registration_ok,
            "cf_readback_ok": cf_readback_ok,
            "query_surfaces_ok": query_surfaces_ok
        }
    });
    write_json_0600(&fsv_path, &evidence).context("write TASK-EK-017 run FSV")?;
    write_json_0600(&canonical_path, &evidence).context("write TASK-EK-017 canonical FSV")?;
    let fsv_sha256 = sha256_file(&fsv_path).context("hash TASK-EK-017 FSV")?;
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "all_passed": all_passed,
            "fsv_path": fsv_path,
            "canonical_path": canonical_path,
            "fsv_sha256": fsv_sha256,
            "top_cross_library_bedrock": engineered_bedrock.chunk_id,
            "custom_library_id": custom.slug()
        }))?
    );
    ensure!(all_passed, "TASK-EK-017 library foundationality FSV failed");
    Ok(())
}

fn registration(
    library_id: LibraryId,
    description: &str,
    registered_at_unix_ms: i64,
    source_ref: &str,
) -> LibraryRegistration {
    LibraryRegistration::new(library_id, description, registered_at_unix_ms, source_ref)
}

fn single_library_fixture_edges() -> Vec<ChunkDependencyEdge> {
    let py = LibraryId::PythonSweBenchLite;
    vec![
        ChunkDependencyEdge::new_with_libraries(
            py.clone(),
            "py:api",
            py.clone(),
            "py:core",
            "call",
            1.0,
            "fixture:py-api-core",
        ),
        ChunkDependencyEdge::new_with_libraries(
            py.clone(),
            "py:test",
            py.clone(),
            "py:api",
            "test",
            1.0,
            "fixture:py-test-api",
        ),
        ChunkDependencyEdge::new_with_libraries(
            py.clone(),
            "py:worker",
            py.clone(),
            "py:core",
            "call",
            0.8,
            "fixture:py-worker-core",
        ),
        ChunkDependencyEdge::new_with_libraries(
            py.clone(),
            "py:test",
            py.clone(),
            "py:worker",
            "test",
            0.6,
            "fixture:py-test-worker",
        ),
    ]
}

fn two_library_fixture_edges() -> Vec<ChunkDependencyEdge> {
    let py = LibraryId::PythonSweBenchLite;
    let shakespeare = LibraryId::ShakespeareCanon;
    let mut edges = single_library_fixture_edges();
    edges.extend([
        ChunkDependencyEdge::new_with_libraries(
            shakespeare.clone(),
            "shake:scene",
            shakespeare.clone(),
            "shake:canon",
            "quote",
            1.0,
            "fixture:shake-scene-canon",
        ),
        ChunkDependencyEdge::new_with_libraries(
            shakespeare.clone(),
            "shake:commentary",
            shakespeare.clone(),
            "shake:scene",
            "quote",
            1.0,
            "fixture:shake-commentary-scene",
        ),
        ChunkDependencyEdge::new_with_libraries(
            shakespeare.clone(),
            "shake:canon",
            py.clone(),
            "py:core",
            "cross_quote",
            2.0,
            "fixture:shake-canon-py-core",
        ),
        ChunkDependencyEdge::new_with_libraries(
            shakespeare,
            "shake:scene",
            py,
            "py:core",
            "cross_quote",
            1.0,
            "fixture:shake-scene-py-core",
        ),
    ]);
    edges
}

fn three_library_fixture_edges() -> Vec<ChunkDependencyEdge> {
    let py = LibraryId::PythonSweBenchLite;
    let custom = LibraryId::Custom("operator-demo".to_string());
    let mut edges = two_library_fixture_edges();
    edges.extend([
        ChunkDependencyEdge::new_with_libraries(
            custom.clone(),
            "custom:workflow",
            custom.clone(),
            "custom:policy",
            "call",
            1.0,
            "fixture:custom-workflow-policy",
        ),
        ChunkDependencyEdge::new_with_libraries(
            custom.clone(),
            "custom:agent",
            custom.clone(),
            "custom:workflow",
            "call",
            1.0,
            "fixture:custom-agent-workflow",
        ),
        ChunkDependencyEdge::new_with_libraries(
            custom.clone(),
            "custom:policy",
            py,
            "py:core",
            "cross_reference",
            1.5,
            "fixture:custom-policy-py-core",
        ),
    ]);
    edges
}

fn sha256_file(path: &Path) -> Result<String> {
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    Ok(hex::encode(Sha256::digest(&bytes)))
}
