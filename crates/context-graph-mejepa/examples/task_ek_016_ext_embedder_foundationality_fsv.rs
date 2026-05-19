use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{ensure, Context, Result};
use context_graph_mejepa::{
    apply_embedder_foundationality_fisher_multiplier, compute_embedder_foundationality, count_cf,
    current_unix_ms, guard_dynamic_embedder_evictions_by_foundationality, open_infer_rocksdb,
    persist_embedder_foundationality_report_sync_readback, plan_dynamic_embedder_evictions,
    read_all_embedder_dependency_edges, read_all_embedder_foundationality_scores,
    read_embedder_foundationality_score, write_dynamic_embedder_registry_sync_readback,
    write_json_0600, ChunkFoundationalityConfig, DynamicEmbedderKind,
    DynamicEmbedderRegistryRecord, DynamicEmbedderUtilityScore, DynamicEmbedderVramBudget,
    EmbedderDependencyEdge, RuntimeEmbedderId, RuntimeRoutingTable,
    MEJEPA_EMBEDDER_FOUNDATIONALITY_EVICTION_BLOCKED,
};
use context_graph_mejepa_cf::{
    CF_MEJEPA_EMBEDDER_DEPENDENCY_GRAPH, CF_MEJEPA_EMBEDDER_FOUNDATIONALITY,
};
use serde_json::json;
use sha2::{Digest, Sha256};

const FSV_ROOT: &str = "/zfs/archive/contextgraph/fsv/task-ek-016-ext-embedder-foundationality-fsv";

fn main() -> Result<()> {
    let started_at_unix_ms = current_unix_ms();
    let run_root = PathBuf::from(FSV_ROOT).join(format!("run-{started_at_unix_ms}"));
    let db_path = run_root.join("infer-rocksdb");
    let fsv_path = run_root.join("embedder_foundationality_fsv.json");
    let canonical_path = PathBuf::from(FSV_ROOT).join("embedder_foundationality_fsv.json");
    fs::create_dir_all(&run_root).context("create TASK-EK-016-ext FSV dir")?;

    let edges = fixture_edges()?;
    let report = compute_embedder_foundationality(
        &edges,
        started_at_unix_ms,
        ChunkFoundationalityConfig::default(),
    )
    .context("compute embedder foundationality")?;
    let protected_id = dynamic(1, "corpus_rotate_v1")?;
    let protected_score = report
        .scores
        .iter()
        .find(|score| score.embedder_id == protected_id.slug())
        .context("protected dynamic embedder score")?
        .clone();
    ensure!(
        protected_score.foundationality_score >= 0.75,
        "fixture protected embedder must clear the guard threshold"
    );

    let db = open_infer_rocksdb(&db_path).context("open embedder-foundationality DB")?;
    let edge_count_before = count_cf(db.as_ref(), CF_MEJEPA_EMBEDDER_DEPENDENCY_GRAPH)?;
    let score_count_before = count_cf(db.as_ref(), CF_MEJEPA_EMBEDDER_FOUNDATIONALITY)?;
    persist_embedder_foundationality_report_sync_readback(db.as_ref(), &edges, &report)
        .context("persist embedder foundationality report")?;
    let direct_edges = read_all_embedder_dependency_edges(db.as_ref())?;
    let direct_scores = read_all_embedder_foundationality_scores(db.as_ref())?;
    let direct_protected =
        read_embedder_foundationality_score(db.as_ref(), protected_id.slug().as_ref())?
            .context("direct protected score")?;

    for record in [
        registry_record(1, "corpus_rotate_v1", 300, true)?,
        registry_record(2, "syntax_head", 300, true)?,
        registry_record(3, "resource_head", 300, true)?,
    ] {
        write_dynamic_embedder_registry_sync_readback(db.as_ref(), &record)
            .context("write dynamic registry record")?;
    }
    let table = RuntimeRoutingTable::new(
        2,
        vec![
            registry_record(1, "corpus_rotate_v1", 300, true)?,
            registry_record(2, "syntax_head", 300, true)?,
            registry_record(3, "resource_head", 300, true)?,
        ],
    )
    .context("runtime routing table")?;
    let budget = DynamicEmbedderVramBudget {
        static_required_bytes: 500,
        budget_bytes: 1100,
    };
    let base_eviction_plan = plan_dynamic_embedder_evictions(
        &table,
        budget,
        &[
            utility(1, "corpus_rotate_v1", 20, 0.01)?,
            utility(2, "syntax_head", 100, 0.60)?,
            utility(3, "resource_head", 90, 0.50)?,
        ],
    )
    .context("base dynamic embedder eviction plan")?;
    let blocked_guard = guard_dynamic_embedder_evictions_by_foundationality(
        base_eviction_plan.clone(),
        &report.scores,
        0.75,
        false,
    )
    .context("blocked foundationality eviction guard")?;
    let approved_guard = guard_dynamic_embedder_evictions_by_foundationality(
        base_eviction_plan.clone(),
        &report.scores,
        0.75,
        true,
    )
    .context("approved foundationality eviction guard")?;

    let protected_weighted_fisher = apply_embedder_foundationality_fisher_multiplier(
        2.0,
        protected_score.foundationality_score,
        2.0,
    )
    .context("protected weighted Fisher")?;
    let leaf_score = report
        .scores
        .last()
        .context("leaf score")?
        .foundationality_score;
    let leaf_weighted_fisher =
        apply_embedder_foundationality_fisher_multiplier(2.0, leaf_score, 2.0)
            .context("leaf weighted Fisher")?;

    drop(db);
    let reopened = open_infer_rocksdb(&db_path).context("reopen embedder-foundationality DB")?;
    let reopened_edges = read_all_embedder_dependency_edges(reopened.as_ref())?;
    let reopened_scores = read_all_embedder_foundationality_scores(reopened.as_ref())?;
    let reopened_protected =
        read_embedder_foundationality_score(reopened.as_ref(), protected_id.slug().as_ref())?
            .context("reopened protected score")?;
    let edge_count_after = count_cf(reopened.as_ref(), CF_MEJEPA_EMBEDDER_DEPENDENCY_GRAPH)?;
    let score_count_after = count_cf(reopened.as_ref(), CF_MEJEPA_EMBEDDER_FOUNDATIONALITY)?;

    let report_score_by_id = report
        .scores
        .iter()
        .map(|score| (score.embedder_id.clone(), score.clone()))
        .collect::<BTreeMap<_, _>>();
    let cf_readback_ok = direct_edges == reopened_edges
        && direct_scores.len() == report.scores.len()
        && reopened_scores.len() == report.scores.len()
        && reopened_scores
            .iter()
            .all(|score| report_score_by_id.get(&score.embedder_id) == Some(score))
        && direct_protected == reopened_protected
        && edge_count_after == edges.len() as u64
        && score_count_after == report.scores.len() as u64;
    let hierarchy_ok = report.scores[0].embedder_id == protected_id.slug()
        && protected_score.foundationality_score > leaf_score;
    let fisher_ok = protected_weighted_fisher > leaf_weighted_fisher;
    let eviction_guard_ok = base_eviction_plan.evicted_ids == vec![protected_id.clone()]
        && blocked_guard.blocked
        && blocked_guard.reason_code.as_deref()
            == Some(MEJEPA_EMBEDDER_FOUNDATIONALITY_EVICTION_BLOCKED)
        && blocked_guard.protected_eviction_ids == vec![protected_id.clone()]
        && !approved_guard.blocked
        && approved_guard.protected_eviction_ids == vec![protected_id.clone()];
    let all_passed = hierarchy_ok && cf_readback_ok && fisher_ok && eviction_guard_ok;

    let evidence = json!({
        "task": "TASK-EK-016-ext",
        "issue": 349,
        "passes": all_passed,
        "all_passed": all_passed,
        "sot": {
            "dependency_graph_cf": CF_MEJEPA_EMBEDDER_DEPENDENCY_GRAPH,
            "foundationality_cf": CF_MEJEPA_EMBEDDER_FOUNDATIONALITY,
            "db_path": db_path,
            "fsv_path": fsv_path,
            "canonical_path": canonical_path,
            "inner_llm_invoked": false
        },
        "graph": {
            "dependency_graph_sha256": report.dependency_graph_sha256,
            "node_count": report.node_count,
            "edge_count": report.edge_count,
            "top_embedder": report.scores.first(),
            "protected_embedder": protected_score,
            "leaf_score": leaf_score
        },
        "readback": {
            "edge_count_before": edge_count_before,
            "score_count_before": score_count_before,
            "direct_edges": direct_edges.len(),
            "direct_scores": direct_scores.len(),
            "reopened_edges": reopened_edges.len(),
            "reopened_scores": reopened_scores.len(),
            "cf_readback_ok": cf_readback_ok
        },
        "fisher": {
            "lambda_fisher": 2.0,
            "protected_weighted_fisher": protected_weighted_fisher,
            "leaf_weighted_fisher": leaf_weighted_fisher,
            "fisher_ok": fisher_ok
        },
        "eviction_guard": {
            "budget": budget,
            "base_plan": base_eviction_plan,
            "blocked_guard": blocked_guard,
            "approved_guard": approved_guard,
            "eviction_guard_ok": eviction_guard_ok
        },
        "checks": {
            "hierarchy_ok": hierarchy_ok,
            "cf_readback_ok": cf_readback_ok,
            "fisher_ok": fisher_ok,
            "eviction_guard_ok": eviction_guard_ok
        }
    });
    write_json_0600(&fsv_path, &evidence).context("write TASK-EK-016-ext run FSV")?;
    write_json_0600(&canonical_path, &evidence).context("write TASK-EK-016-ext canonical FSV")?;
    let fsv_sha256 = sha256_file(&fsv_path).context("hash TASK-EK-016-ext FSV")?;
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "all_passed": all_passed,
            "fsv_path": fsv_path,
            "canonical_path": canonical_path,
            "fsv_sha256": fsv_sha256,
            "protected_embedder": protected_id.slug(),
            "protected_score": protected_score.foundationality_score,
        }))?
    );
    ensure!(
        all_passed,
        "TASK-EK-016-ext embedder foundationality FSV failed"
    );
    Ok(())
}

fn fixture_edges() -> Result<Vec<EmbedderDependencyEdge>> {
    Ok(vec![
        EmbedderDependencyEdge::new(
            dynamic(2, "syntax_head")?,
            dynamic(1, "corpus_rotate_v1")?,
            0.03,
            "heldout:q1:syntax_without_rotate_delta",
        ),
        EmbedderDependencyEdge::new(
            dynamic(3, "resource_head")?,
            dynamic(1, "corpus_rotate_v1")?,
            0.04,
            "heldout:q2:resource_without_rotate_delta",
        ),
        EmbedderDependencyEdge::new(
            dynamic(4, "test_head")?,
            dynamic(1, "corpus_rotate_v1")?,
            0.02,
            "heldout:q3:test_without_rotate_delta",
        ),
        EmbedderDependencyEdge::new(
            dynamic(2, "syntax_head")?,
            "e7".parse::<RuntimeEmbedderId>()?,
            0.01,
            "heldout:q1:syntax_without_e7_delta",
        ),
        EmbedderDependencyEdge::new(
            dynamic(3, "resource_head")?,
            "e8".parse::<RuntimeEmbedderId>()?,
            0.01,
            "heldout:q2:resource_without_e8_delta",
        ),
    ])
}

fn registry_record(
    sequence: u32,
    name: &str,
    required_vram_bytes: u64,
    active: bool,
) -> Result<DynamicEmbedderRegistryRecord> {
    let record = DynamicEmbedderRegistryRecord {
        id: dynamic(sequence, name)?,
        registry_version: 2,
        kind: DynamicEmbedderKind::LearnedHead,
        dimension: 128,
        route_languages: vec!["python".to_string()],
        route_entity_types: vec!["*".to_string()],
        forward_artifact_path: format!(
            "/zfs/archive/contextgraph/models/dynamic/edynamic_{sequence}_{name}/forward.bin"
        ),
        forward_artifact_sha256: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
            .to_string(),
        required_vram_bytes,
        active,
        promoted_at_unix_ms: 1_779_100_000_000 + i64::from(sequence),
    };
    record.validate()?;
    Ok(record)
}

fn utility(
    sequence: u32,
    name: &str,
    last_used_unix_ms: i64,
    mean_per_cell_contribution: f32,
) -> Result<DynamicEmbedderUtilityScore> {
    let utility = DynamicEmbedderUtilityScore {
        id: dynamic(sequence, name)?,
        last_used_unix_ms,
        mean_per_cell_contribution,
    };
    utility.validate()?;
    Ok(utility)
}

fn dynamic(sequence: u32, name: &str) -> Result<RuntimeEmbedderId> {
    RuntimeEmbedderId::dynamic(sequence, name).map_err(Into::into)
}

fn sha256_file(path: &Path) -> Result<String> {
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(hex::encode(hasher.finalize()))
}
