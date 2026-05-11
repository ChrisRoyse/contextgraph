// Inspired by ruvnet/RuVector crates/ruvector-solver/src/forward_push.rs at HEAD ef5274c2 (read 2026-05-08).
// Clean-room integration; no code copied, no upstream tracking. See
// memory/decisions/agent-141-coordinator--upstream-reference-only-clean-room.md
// for the policy.

//! Graph-backed influence ranking for ccreality optimizer evidence.
//!
//! Source of truth:
//! - `<runtime_root>/<run_id>/reality-optimizer/attempts.jsonl`
//! - `recommendation-turn-*.json` files under `<runtime_root>/<run_id>/`
//! - persisted computation audit at
//!   `<runtime_root>/<run_id>/reality-optimizer/influence/influence-*.json`

use super::errors::{CCRealityError, Result};
use super::helpers::*;
use super::witness_chain::{
    append_witness_entry_for_run, verify_witness_chain_for_run, WitnessOpType,
};
use crate::handlers::Handlers;
use crate::protocol::{JsonRpcId, JsonRpcResponse};
use context_graph_solver::{CsrMatrix, ForwardPushConfig, ForwardPushSolver, MatrixKind};
use serde_json::{json, Value};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

const DEFAULT_K: u64 = 5;
const MAX_K: u64 = 100;
const DEFAULT_ALPHA: f64 = 0.15;
const DEFAULT_TOLERANCE: f64 = 1e-8;
const DEFAULT_MAX_PUSHES: u64 = 1_000_000;

impl Handlers {
    pub(crate) async fn call_optimizer_compute_influence(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        match optimizer_compute_influence(args).await {
            Ok(v) => super::ok(self, id, v),
            Err(e) => super::err(id, e),
        }
    }
}

pub async fn optimizer_compute_influence(args: Value) -> Result<Value> {
    let args = coerce_stringified_args(args);
    let runtime_root = require_active_runtime_root().await?;
    let run_id = match optional_str_strict(&args, "run_id")? {
        Some(run_id) => run_id,
        None => latest_run_id(&runtime_root)?.ok_or_else(|| {
            CCRealityError::new(
                "CCREALITY_INFLUENCE_NO_ACTIVE_RUN",
                "no active run under the runtime root",
                "active_run",
                "run a reality-loop attempt before computing graph influence",
                json!({"runtime_root": runtime_root.display().to_string()}),
                Some(file_sot(&runtime_root)),
            )
        })?,
    };
    compute_influence_for_run(&runtime_root, &run_id, &args)
}

pub(crate) fn compute_influence_for_run(
    runtime_root: &Path,
    run_id: &str,
    args: &Value,
) -> Result<Value> {
    let failure_tag = required_str(args, "failure_tag")?;
    let k = optional_u64_strict(args, "k", DEFAULT_K)?.min(MAX_K) as usize;
    if k == 0 {
        return Err(CCRealityError::new(
            "CCREALITY_INFLUENCE_K_INVALID",
            "k must be greater than zero",
            "arguments.k",
            "request at least one influenced attempt or recommendation",
            json!({"k": args.get("k")}),
            None,
        ));
    }
    let alpha = optional_f64_strict(args, "alpha", DEFAULT_ALPHA)?;
    let tolerance = optional_f64_strict(args, "tolerance", DEFAULT_TOLERANCE)?;
    let max_pushes = optional_u64_strict(args, "max_pushes", DEFAULT_MAX_PUSHES)? as usize;
    let solver = ForwardPushSolver::new(ForwardPushConfig {
        alpha,
        tolerance,
        max_pushes,
    })
    .map_err(solver_error)?;

    let run_root = runtime_root.join(run_id);
    let witness_preflight = verify_witness_chain_for_run(runtime_root, run_id)?;
    let attempts_path = run_root.join("reality-optimizer").join("attempts.jsonl");
    let attempts = read_attempt_records(&attempts_path)?;
    let mut recommendation_paths = Vec::new();
    collect_recommendation_paths(&run_root, &mut recommendation_paths)?;
    if recommendation_paths.is_empty() {
        return Err(CCRealityError::new(
            "CCREALITY_INFLUENCE_RECOMMENDATIONS_MISSING",
            "no recommendation-turn-*.json files were found under the run root",
            "recommendations.path",
            "record optimizer recommendations before computing graph influence",
            json!({"run_root": run_root.display().to_string()}),
            Some(file_sot(&run_root)),
        ));
    }
    let recommendations = read_recommendation_records(&run_root, &recommendation_paths)?;
    let graph = build_graph(&failure_tag, attempts, recommendations)?;
    let csr = CsrMatrix::from_edges(
        graph.nodes.len(),
        graph.nodes.len(),
        MatrixKind::NonNegativeAdjacency,
        &graph.edges,
    )
    .map_err(solver_error)?;
    let report = solver
        .solve_from_seed(&csr, graph.seed_node)
        .map_err(solver_error)?;

    let top_attempts = top_attempts(&graph, &report.estimate, k);
    let top_recommendations = top_recommendations(&graph, &report.estimate, k)?;
    if top_attempts.is_empty() && top_recommendations.is_empty() {
        return Err(CCRealityError::new(
            "CCREALITY_INFLUENCE_NO_REACHABLE_EVIDENCE",
            "PPR did not reach any attempt or recommendation evidence from the failure tag",
            "influence.result",
            "inspect graph tags and recommendation linkage before trusting influence ranking",
            json!({"failure_tag": failure_tag, "seed_node": graph.seed_node_id}),
            Some(file_sot(&attempts_path)),
        ));
    }

    let graph_evidence = graph_evidence_json(&graph, &attempts_path, &recommendation_paths)?;
    let audit = canonical_json(json!({
        "schema_version": 1,
        "record_kind": "ccreality_optimizer_influence_computation",
        "run_id": run_id,
        "failure_tag": failure_tag,
        "algorithm": "forward_push_personalized_pagerank",
        "graph_model": "undirected_tag_attempt_recommendation_bipartite",
        "parameters": {
            "k": k,
            "alpha": alpha,
            "tolerance": tolerance,
            "max_pushes": max_pushes
        },
        "graph": graph_evidence,
        "solver_report": {
            "pushes": report.pushes,
            "residual_l1": report.residual_l1,
            "estimate_l1": report.estimate_l1,
            "total_mass": report.total_mass
        },
        "top_attempts": top_attempts,
        "top_recommendations": top_recommendations,
        "witness_preflight": witness_preflight,
        "created_at_unix": unix_secs()?
    }))?;
    let audit_path = next_influence_audit_path(&run_root)?;
    write_json_checked(&audit_path, &audit)?;
    let audit_readback = read_json(&audit_path)?;
    if audit_readback != audit {
        return Err(CCRealityError::new(
            "CCREALITY_INFLUENCE_AUDIT_READBACK_MISMATCH",
            "influence audit readback did not match the computed payload",
            "influence.audit.readback",
            "inspect filesystem durability before trusting influence output",
            json!({"path": audit_path.display().to_string()}),
            Some(file_sot(&audit_path)),
        ));
    }
    let audit_sha256 = sha256_file(&audit_path)?;
    let witness_append = append_witness_entry_for_run(
        runtime_root,
        run_id,
        WitnessOpType::InfluenceComputation,
        &audit_sha256,
    )?;
    Ok(json!({
        "status": "ok",
        "run_id": run_id,
        "failure_tag": failure_tag,
        "algorithm": "forward_push_personalized_pagerank",
        "source_of_truth": {
            "run_root": file_sot(&run_root),
            "attempts_jsonl": file_sot(&attempts_path),
            "influence_audit": file_sot(&audit_path),
            "witness_chain": file_sot(&runtime_root.join(run_id).join("claude-code-optimizer").join("witness-chain.bin"))
        },
        "attempts_sha256": sha256_file(&attempts_path)?,
        "influence_audit_sha256": audit_sha256,
        "witness_append": witness_append,
        "graph": audit["graph"].clone(),
        "solver_report": audit["solver_report"].clone(),
        "top_attempts": audit["top_attempts"].clone(),
        "top_recommendations": audit["top_recommendations"].clone()
    }))
}

#[derive(Debug, Clone)]
struct AttemptRecord {
    key: AttemptKey,
    repo: Option<String>,
    tags: Vec<String>,
    row: Value,
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
struct AttemptKey {
    instance_id: String,
    attempt: u64,
}

impl AttemptKey {
    fn node_id(&self) -> String {
        format!("attempt:{}#{}", self.instance_id, self.attempt)
    }
}

#[derive(Debug, Clone)]
struct RecommendationRecord {
    key: AttemptKey,
    path: PathBuf,
    relative_path: String,
    body: Value,
}

#[derive(Debug, Clone)]
enum NodeKind {
    Tag { tag: String },
    Attempt { key: AttemptKey },
    Recommendation { path: String },
}

#[derive(Debug)]
struct InfluenceGraph {
    nodes: Vec<NodeKind>,
    node_ids: Vec<String>,
    node_index: BTreeMap<String, usize>,
    seed_node: usize,
    seed_node_id: String,
    edges: Vec<(usize, usize, f64)>,
    attempts: BTreeMap<AttemptKey, AttemptRecord>,
    recommendations: BTreeMap<String, RecommendationRecord>,
    attempt_recommendations: BTreeMap<AttemptKey, Vec<String>>,
    known_tags: Vec<String>,
}

fn read_attempt_records(path: &Path) -> Result<BTreeMap<AttemptKey, AttemptRecord>> {
    if !path.is_file() {
        return Err(CCRealityError::new(
            "CCREALITY_INFLUENCE_ATTEMPTS_MISSING",
            "attempts.jsonl is missing from the optimizer state directory",
            "attempts.path",
            "record ME-JEPA outer-loop evidence so attempts.jsonl exists",
            json!({"path": path.display().to_string()}),
            Some(file_sot(path)),
        ));
    }
    let raw = fs::read_to_string(path)
        .map_err(|e| fs_error("CCREALITY_INFLUENCE_ATTEMPTS_READ_FAILED", path, e))?;
    let mut out = BTreeMap::new();
    for (line_idx, line) in raw.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let row: Value = serde_json::from_str(line).map_err(|err| {
            CCRealityError::new(
                "CCREALITY_INFLUENCE_ATTEMPTS_JSON_INVALID",
                format!(
                    "invalid attempts.jsonl JSON at line {}: {err}",
                    line_idx + 1
                ),
                "attempts.parse",
                "repair or remove the corrupt attempts.jsonl line",
                json!({"path": path.display().to_string(), "line": line_idx + 1}),
                Some(file_sot(path)),
            )
        })?;
        let instance_id = require_row_str(path, line_idx + 1, &row, "instance_id")?;
        let attempt = require_row_u64(path, line_idx + 1, &row, "attempt")?;
        let tags = require_tags(path, line_idx + 1, &row)?;
        let key = AttemptKey {
            instance_id,
            attempt,
        };
        if out.contains_key(&key) {
            return Err(CCRealityError::new(
                "CCREALITY_INFLUENCE_ATTEMPT_DUPLICATE",
                "attempts.jsonl contains a duplicate (instance_id, attempt) row",
                "attempts.key",
                "deduplicate attempts.jsonl before computing influence",
                json!({"path": path.display().to_string(), "line": line_idx + 1, "instance_id": key.instance_id, "attempt": key.attempt}),
                Some(file_sot(path)),
            ));
        }
        out.insert(
            key.clone(),
            AttemptRecord {
                key,
                repo: row
                    .get("repo")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned),
                tags,
                row,
            },
        );
    }
    if out.is_empty() {
        return Err(CCRealityError::new(
            "CCREALITY_INFLUENCE_ATTEMPTS_EMPTY",
            "attempts.jsonl contains no attempt rows",
            "attempts.rows",
            "record ME-JEPA outer-loop evidence before computing influence",
            json!({"path": path.display().to_string()}),
            Some(file_sot(path)),
        ));
    }
    Ok(out)
}

fn require_row_str(path: &Path, line: usize, row: &Value, field: &str) -> Result<String> {
    row.get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| {
            CCRealityError::new(
                "CCREALITY_INFLUENCE_ATTEMPT_FIELD_INVALID",
                format!("attempts.jsonl line {line} field '{field}' must be a non-empty string"),
                format!("attempts.line{line}.{field}"),
                "repair attempts.jsonl before computing graph influence",
                json!({"path": path.display().to_string(), "line": line, "row": row}),
                Some(file_sot(path)),
            )
        })
}

fn require_row_u64(path: &Path, line: usize, row: &Value, field: &str) -> Result<u64> {
    row.get(field).and_then(Value::as_u64).ok_or_else(|| {
        CCRealityError::new(
            "CCREALITY_INFLUENCE_ATTEMPT_FIELD_INVALID",
            format!("attempts.jsonl line {line} field '{field}' must be a non-negative integer"),
            format!("attempts.line{line}.{field}"),
            "repair attempts.jsonl before computing graph influence",
            json!({"path": path.display().to_string(), "line": line, "row": row}),
            Some(file_sot(path)),
        )
    })
}

fn require_tags(path: &Path, line: usize, row: &Value) -> Result<Vec<String>> {
    let Some(values) = row.get("tags").and_then(Value::as_array) else {
        return Err(CCRealityError::new(
            "CCREALITY_INFLUENCE_TAGS_INVALID",
            "attempt row does not contain a tags array",
            format!("attempts.line{line}.tags"),
            "write Reflexion tags before computing influence",
            json!({"path": path.display().to_string(), "line": line, "row": row}),
            Some(file_sot(path)),
        ));
    };
    let mut tags = BTreeSet::new();
    for value in values {
        let Some(tag) = value.as_str().filter(|tag| !tag.is_empty()) else {
            return Err(CCRealityError::new(
                "CCREALITY_INFLUENCE_TAGS_INVALID",
                "attempt tags must contain only non-empty strings",
                format!("attempts.line{line}.tags"),
                "repair attempt tags before computing influence",
                json!({"path": path.display().to_string(), "line": line, "tags": values}),
                Some(file_sot(path)),
            ));
        };
        tags.insert(tag.to_string());
    }
    if tags.is_empty() {
        return Err(CCRealityError::new(
            "CCREALITY_INFLUENCE_TAGS_EMPTY",
            "attempt row tags array is empty",
            format!("attempts.line{line}.tags"),
            "write at least one failure/outcome tag before computing influence",
            json!({"path": path.display().to_string(), "line": line, "row": row}),
            Some(file_sot(path)),
        ));
    }
    Ok(tags.into_iter().collect())
}

fn collect_recommendation_paths(root: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    if !root.is_dir() {
        return Err(CCRealityError::new(
            "CCREALITY_INFLUENCE_RUN_ROOT_MISSING",
            "run root is missing or is not a directory",
            "run_root",
            "verify the active runtime root and run_id before computing influence",
            json!({"run_root": root.display().to_string()}),
            Some(file_sot(root)),
        ));
    }
    for entry in fs::read_dir(root)
        .map_err(|e| fs_error("CCREALITY_INFLUENCE_RECOMMENDATION_WALK_FAILED", root, e))?
    {
        let entry = entry
            .map_err(|e| fs_error("CCREALITY_INFLUENCE_RECOMMENDATION_ENTRY_FAILED", root, e))?;
        let path = entry.path();
        if path.is_dir() {
            collect_recommendation_paths(&path, out)?;
        } else if path
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.starts_with("recommendation-turn-") && name.ends_with(".json"))
            .unwrap_or(false)
        {
            out.push(path);
        }
    }
    out.sort();
    Ok(())
}

fn read_recommendation_records(
    run_root: &Path,
    paths: &[PathBuf],
) -> Result<Vec<RecommendationRecord>> {
    let mut out = Vec::new();
    for path in paths {
        let body = read_json(path)?;
        let path_key = attempt_key_from_recommendation_path(run_root, path)?;
        let body_attempt = body.get("attempt").and_then(Value::as_u64).ok_or_else(|| {
            CCRealityError::new(
                "CCREALITY_INFLUENCE_RECOMMENDATION_ATTEMPT_INVALID",
                "recommendation file does not contain a numeric attempt field",
                "recommendation.attempt",
                "record optimizer recommendations with the attempt number before computing influence",
                json!({"path": path.display().to_string(), "body": body}),
                Some(file_sot(path)),
            )
        })?;
        if body_attempt != path_key.attempt {
            return Err(CCRealityError::new(
                "CCREALITY_INFLUENCE_RECOMMENDATION_ATTEMPT_MISMATCH",
                "recommendation path attempt does not match recommendation.attempt",
                "recommendation.attempt",
                "move or regenerate the recommendation file before computing influence",
                json!({"path": path.display().to_string(), "path_attempt": path_key.attempt, "body_attempt": body_attempt}),
                Some(file_sot(path)),
            ));
        }
        let relative_path = path
            .strip_prefix(run_root)
            .map_err(|_| {
                CCRealityError::new(
                    "CCREALITY_INFLUENCE_RECOMMENDATION_PATH_INVALID",
                    "recommendation path is not under the run root",
                    "recommendation.path",
                    "inspect recommendation path collection before computing influence",
                    json!({"run_root": run_root.display().to_string(), "path": path.display().to_string()}),
                    Some(file_sot(path)),
                )
            })?
            .display()
            .to_string();
        out.push(RecommendationRecord {
            key: path_key,
            path: path.clone(),
            relative_path,
            body,
        });
    }
    Ok(out)
}

fn attempt_key_from_recommendation_path(run_root: &Path, path: &Path) -> Result<AttemptKey> {
    let relative = path.strip_prefix(run_root).map_err(|_| {
        CCRealityError::new(
            "CCREALITY_INFLUENCE_RECOMMENDATION_PATH_INVALID",
            "recommendation path is not under the run root",
            "recommendation.path",
            "inspect recommendation path collection before computing influence",
            json!({"run_root": run_root.display().to_string(), "path": path.display().to_string()}),
            Some(file_sot(path)),
        )
    })?;
    let components = relative
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => value.to_str().map(ToOwned::to_owned),
            _ => None,
        })
        .collect::<Vec<_>>();
    for index in 1..components.len() {
        if let Some(attempt) = parse_attempt_component(&components[index]) {
            return Ok(AttemptKey {
                instance_id: components[index - 1].clone(),
                attempt,
            });
        }
    }
    Err(CCRealityError::new(
        "CCREALITY_INFLUENCE_RECOMMENDATION_ATTEMPT_PATH_MISSING",
        "recommendation path is not nested below an attempt-N or smoke-attempt-N directory",
        "recommendation.path",
        "store recommendations under <run>/<instance>/attempt-N/claude-code-optimizer/",
        json!({"run_root": run_root.display().to_string(), "path": path.display().to_string()}),
        Some(file_sot(path)),
    ))
}

fn parse_attempt_component(name: &str) -> Option<u64> {
    name.strip_prefix("attempt-")
        .or_else(|| name.strip_prefix("smoke-attempt-"))
        .and_then(|suffix| suffix.parse::<u64>().ok())
}

fn build_graph(
    failure_tag: &str,
    attempts: BTreeMap<AttemptKey, AttemptRecord>,
    recommendations: Vec<RecommendationRecord>,
) -> Result<InfluenceGraph> {
    let mut graph = InfluenceGraph {
        nodes: Vec::new(),
        node_ids: Vec::new(),
        node_index: BTreeMap::new(),
        seed_node: 0,
        seed_node_id: format!("tag:{failure_tag}"),
        edges: Vec::new(),
        attempts,
        recommendations: BTreeMap::new(),
        attempt_recommendations: BTreeMap::new(),
        known_tags: Vec::new(),
    };
    let mut known_tags = BTreeSet::new();
    let attempts_snapshot = graph.attempts.values().cloned().collect::<Vec<_>>();
    for attempt in attempts_snapshot {
        let attempt_id = attempt.key.node_id();
        ensure_node(
            &mut graph,
            attempt_id.clone(),
            NodeKind::Attempt {
                key: attempt.key.clone(),
            },
        );
        for tag in &attempt.tags {
            known_tags.insert(tag.clone());
            let tag_id = format!("tag:{tag}");
            ensure_node(
                &mut graph,
                tag_id.clone(),
                NodeKind::Tag { tag: tag.clone() },
            );
            add_undirected_edge(&mut graph, &tag_id, &attempt_id, 1.0);
        }
    }
    graph.known_tags = known_tags.into_iter().collect();
    if !graph.node_index.contains_key(&graph.seed_node_id) {
        return Err(CCRealityError::new(
            "CCREALITY_INFLUENCE_FAILURE_TAG_UNKNOWN",
            "failure_tag is not present in attempts.jsonl tags",
            "arguments.failure_tag",
            "choose a tag from the known_tags list or run more attempts with that failure mode",
            json!({"failure_tag": failure_tag, "known_tags": graph.known_tags}),
            None,
        ));
    }
    graph.seed_node = *graph.node_index.get(&graph.seed_node_id).expect("checked");

    for recommendation in recommendations {
        if !graph.attempts.contains_key(&recommendation.key) {
            return Err(CCRealityError::new(
                "CCREALITY_INFLUENCE_ORPHAN_RECOMMENDATION",
                "recommendation file references an attempt absent from attempts.jsonl",
                "recommendation.attempt",
                "rebuild attempts.jsonl or move the recommendation under the matching attempt before computing influence",
                json!({
                    "recommendation_path": recommendation.path.display().to_string(),
                    "instance_id": recommendation.key.instance_id,
                    "attempt": recommendation.key.attempt
                }),
                Some(file_sot(&recommendation.path)),
            ));
        }
        let rec_id = format!("recommendation:{}", recommendation.relative_path);
        ensure_node(
            &mut graph,
            rec_id.clone(),
            NodeKind::Recommendation {
                path: recommendation.relative_path.clone(),
            },
        );
        let attempt_id = recommendation.key.node_id();
        add_undirected_edge(&mut graph, &attempt_id, &rec_id, 1.0);
        graph
            .attempt_recommendations
            .entry(recommendation.key.clone())
            .or_default()
            .push(recommendation.relative_path.clone());
        graph
            .recommendations
            .insert(recommendation.relative_path.clone(), recommendation);
    }
    Ok(graph)
}

fn ensure_node(graph: &mut InfluenceGraph, id: String, kind: NodeKind) -> usize {
    if let Some(idx) = graph.node_index.get(&id).copied() {
        return idx;
    }
    let idx = graph.nodes.len();
    graph.node_index.insert(id.clone(), idx);
    graph.node_ids.push(id);
    graph.nodes.push(kind);
    idx
}

fn add_undirected_edge(graph: &mut InfluenceGraph, left: &str, right: &str, weight: f64) {
    let left_idx = graph.node_index[left];
    let right_idx = graph.node_index[right];
    graph.edges.push((left_idx, right_idx, weight));
    graph.edges.push((right_idx, left_idx, weight));
}

fn top_attempts(graph: &InfluenceGraph, estimate: &[f64], k: usize) -> Vec<Value> {
    let mut rows = Vec::new();
    for (node_idx, node) in graph.nodes.iter().enumerate() {
        let NodeKind::Attempt { key } = node else {
            continue;
        };
        let score = estimate[node_idx];
        if score <= 0.0 {
            continue;
        }
        if let Some(attempt) = graph.attempts.get(key) {
            rows.push((score, key.clone(), attempt.clone()));
        }
    }
    rows.sort_by(|a, b| {
        b.0.partial_cmp(&a.0)
            .unwrap_or(Ordering::Equal)
            .then_with(|| a.1.cmp(&b.1))
    });
    rows.into_iter()
        .take(k)
        .enumerate()
        .map(|(rank, (score, key, attempt))| {
            json!({
                "rank": rank + 1,
                "influence_score": score,
                "node_id": key.node_id(),
                "instance_id": key.instance_id,
                "attempt": key.attempt,
                "repo": attempt.repo,
                "tags": attempt.tags,
                "outcome_kind": attempt.row.get("outcome_kind").cloned().unwrap_or(Value::Null),
                "reward": attempt.row.get("reward").cloned().unwrap_or(Value::Null),
                "official_resolved": attempt.row.get("official_resolved").cloned().unwrap_or(Value::Null),
                "related_recommendations": graph
                    .attempt_recommendations
                    .get(&attempt.key)
                    .cloned()
                    .unwrap_or_default(),
                "attempt_row": attempt.row
            })
        })
        .collect()
}

fn top_recommendations(graph: &InfluenceGraph, estimate: &[f64], k: usize) -> Result<Vec<Value>> {
    let mut rows = Vec::new();
    for (node_idx, node) in graph.nodes.iter().enumerate() {
        let NodeKind::Recommendation { path } = node else {
            continue;
        };
        let score = estimate[node_idx];
        if score <= 0.0 {
            continue;
        }
        let Some(recommendation) = graph.recommendations.get(path) else {
            return Err(CCRealityError::new(
                "CCREALITY_INFLUENCE_RECOMMENDATION_NODE_ORPHANED",
                "recommendation node has no backing recommendation record",
                "influence.graph.recommendations",
                "inspect graph construction before trusting influence output",
                json!({"path": path}),
                None,
            ));
        };
        rows.push((score, path.clone(), recommendation.clone()));
    }
    rows.sort_by(|a, b| {
        b.0.partial_cmp(&a.0)
            .unwrap_or(Ordering::Equal)
            .then_with(|| a.1.cmp(&b.1))
    });
    rows.into_iter()
        .take(k)
        .enumerate()
        .map(|(rank, (score, path, recommendation))| {
            Ok(json!({
                "rank": rank + 1,
                "influence_score": score,
                "node_id": format!("recommendation:{path}"),
                "recommendation_path": file_sot(&recommendation.path),
                "recommendation_sha256": sha256_file(&recommendation.path)?,
                "instance_id": recommendation.key.instance_id,
                "attempt": recommendation.key.attempt,
                "status": recommendation.body.get("status").cloned().unwrap_or(Value::Null),
                "diagnosis_summary": recommendation.body.get("diagnosis_summary").cloned().unwrap_or(Value::Null),
                "recommendation": recommendation.body
            }))
        })
        .collect()
}

fn graph_evidence_json(
    graph: &InfluenceGraph,
    attempts_path: &Path,
    recommendation_paths: &[PathBuf],
) -> Result<Value> {
    let mut by_kind = BTreeMap::<&'static str, usize>::new();
    for node in &graph.nodes {
        let key = match node {
            NodeKind::Tag { tag } => {
                let _ = tag;
                "tag"
            }
            NodeKind::Attempt { .. } => "attempt",
            NodeKind::Recommendation { .. } => "recommendation",
        };
        *by_kind.entry(key).or_default() += 1;
    }
    let recommendation_hashes = recommendation_paths
        .iter()
        .map(|path| {
            Ok(json!({
                "path": file_sot(path),
                "sha256": sha256_file(path)?
            }))
        })
        .collect::<Result<Vec<_>>>()?;
    let graph_payload = json!({
        "seed_node": graph.seed_node_id,
        "node_count": graph.nodes.len(),
        "edge_count": graph.edges.len(),
        "node_counts": by_kind,
        "known_tags": graph.known_tags,
        "attempts_source": {
            "path": file_sot(attempts_path),
            "sha256": sha256_file(attempts_path)?
        },
        "recommendation_sources": recommendation_hashes
    });
    let graph_fingerprint = sha256_text(&serde_json::to_string(&graph_payload).map_err(|err| {
        CCRealityError::new(
            "CCREALITY_INFLUENCE_GRAPH_SERIALIZE_FAILED",
            format!("failed to serialize graph evidence: {err}"),
            "influence.graph",
            "inspect graph evidence before persisting influence audit",
            json!({}),
            Some(file_sot(attempts_path)),
        )
    })?);
    Ok(json!({
        "fingerprint": graph_fingerprint,
        "details": graph_payload
    }))
}

fn next_influence_audit_path(run_root: &Path) -> Result<PathBuf> {
    let dir = run_root.join("reality-optimizer").join("influence");
    fs::create_dir_all(&dir)
        .map_err(|e| fs_error("CCREALITY_INFLUENCE_AUDIT_DIR_CREATE_FAILED", &dir, e))?;
    let mut max_seq = 0u64;
    for entry in fs::read_dir(&dir)
        .map_err(|e| fs_error("CCREALITY_INFLUENCE_AUDIT_DIR_READ_FAILED", &dir, e))?
    {
        let entry =
            entry.map_err(|e| fs_error("CCREALITY_INFLUENCE_AUDIT_DIR_ENTRY_FAILED", &dir, e))?;
        let name = entry.file_name().to_string_lossy().to_string();
        if let Some(num) = name
            .strip_prefix("influence-")
            .and_then(|s| s.strip_suffix(".json"))
            .and_then(|s| s.parse::<u64>().ok())
        {
            max_seq = max_seq.max(num);
        }
    }
    Ok(dir.join(format!("influence-{:04}.json", max_seq + 1)))
}

fn canonical_json(value: Value) -> Result<Value> {
    let text = serde_json::to_string(&value).map_err(|err| {
        CCRealityError::new(
            "CCREALITY_INFLUENCE_AUDIT_CANONICALIZE_FAILED",
            format!("failed to serialize influence audit for canonical readback: {err}"),
            "influence.audit",
            "inspect influence audit payload before writing it to disk",
            json!({}),
            None,
        )
    })?;
    serde_json::from_str(&text).map_err(|err| {
        CCRealityError::new(
            "CCREALITY_INFLUENCE_AUDIT_CANONICALIZE_FAILED",
            format!("failed to parse canonical influence audit: {err}"),
            "influence.audit",
            "inspect influence audit payload before writing it to disk",
            json!({}),
            None,
        )
    })
}

fn optional_f64_strict(args: &Value, field: &str, default: f64) -> Result<f64> {
    let value = match args.get(field) {
        None | Some(Value::Null) => return Ok(default),
        Some(Value::Number(number)) => number
            .as_f64()
            .ok_or_else(|| invalid_f64(field, args.get(field)))?,
        Some(Value::String(text)) => text
            .trim()
            .parse::<f64>()
            .map_err(|_| invalid_f64(field, args.get(field)))?,
        Some(_) => return Err(invalid_f64(field, args.get(field))),
    };
    if !value.is_finite() {
        return Err(invalid_f64(field, args.get(field)));
    }
    Ok(value)
}

fn invalid_f64(field: &str, value: Option<&Value>) -> CCRealityError {
    CCRealityError::new(
        "CCREALITY_ARG_INVALID_F64",
        format!("argument '{field}' must be a finite number"),
        format!("arguments.{field}"),
        format!("omit arguments.{field} or provide a finite number"),
        json!({"value": value}),
        None,
    )
}

fn solver_error(err: context_graph_solver::SolverError) -> CCRealityError {
    CCRealityError::new(
        format!("CCREALITY_INFLUENCE_{}", err.code()),
        err.to_string(),
        "influence.solver",
        "inspect influence graph construction, PPR parameters, and persisted optimizer evidence",
        json!({"solver_error": err.to_string()}),
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use context_graph_witness::{verify_chain_bytes_with_type_validator, WITNESS_ENTRY_SIZE};
    use tempfile::TempDir;

    fn seed_runtime() -> (TempDir, PathBuf, String) {
        let tmp = TempDir::new().expect("tempdir");
        let runtime_root = tmp.path().join("runtime-root");
        let run_id = "run-test-001".to_string();
        let run_root = runtime_root.join(&run_id);
        fs::create_dir_all(run_root.join("reality-optimizer")).expect("optimizer dir");
        (tmp, runtime_root, run_id)
    }

    fn attempts_path(runtime_root: &Path, run_id: &str) -> PathBuf {
        runtime_root
            .join(run_id)
            .join("reality-optimizer")
            .join("attempts.jsonl")
    }

    fn audit_count(runtime_root: &Path, run_id: &str) -> usize {
        let dir = runtime_root
            .join(run_id)
            .join("reality-optimizer")
            .join("influence");
        if !dir.is_dir() {
            return 0;
        }
        fs::read_dir(dir)
            .expect("read audit dir")
            .filter_map(std::result::Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_str()
                    .map(|name| name.starts_with("influence-") && name.ends_with(".json"))
                    .unwrap_or(false)
            })
            .count()
    }

    fn write_attempts(runtime_root: &Path, run_id: &str, rows: &[Value]) {
        let text = rows
            .iter()
            .map(|row| serde_json::to_string(row).expect("serialize row"))
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        write_text_checked(&attempts_path(runtime_root, run_id), &text).expect("write attempts");
    }

    fn write_recommendation(
        runtime_root: &Path,
        run_id: &str,
        instance_id: &str,
        attempt: u64,
        turn: u64,
        failure_class: &str,
    ) {
        let path = runtime_root
            .join(run_id)
            .join(instance_id)
            .join(format!("attempt-{attempt}"))
            .join("claude-code-optimizer")
            .join(format!("recommendation-turn-{turn:02}.json"));
        let payload = json!({
            "run_id": run_id,
            "attempt": attempt,
            "turn_number": turn,
            "status": "changed",
            "reason": format!("address {failure_class} with a generic harness fix"),
            "diagnosis_summary": {
                "failure_class": failure_class,
                "root_cause_hypothesis": format!("{failure_class} repeats across attempts"),
                "intervention_surface": "tool_surface"
            }
        });
        write_json_checked(&path, &payload).expect("write recommendation");
    }

    fn write_corrupt_witness_chain(runtime_root: &Path, run_id: &str) -> PathBuf {
        let path = runtime_root
            .join(run_id)
            .join("claude-code-optimizer")
            .join("witness-chain.bin");
        fs::create_dir_all(path.parent().expect("witness parent")).expect("witness parent");
        let mut bytes = vec![0u8; WITNESS_ENTRY_SIZE];
        bytes[0] = 0x7f;
        fs::write(&path, bytes).expect("write corrupt witness");
        path
    }

    fn base_rows() -> Vec<Value> {
        vec![
            json!({
                "record_kind": "ccreality_autoresearch_attempt",
                "instance_id": "task-a",
                "repo": "synthetic/repo-a",
                "attempt": 1,
                "task_summary": "import failure task",
                "critique": "validation_status=failed; error_code=IMPORT_MISSING",
                "reward": 0.2,
                "official_resolved": false,
                "outcome_kind": "validation_failed",
                "tags": ["import-error", "validation-failed", "official-unresolved"]
            }),
            json!({
                "record_kind": "ccreality_autoresearch_attempt",
                "instance_id": "task-b",
                "repo": "synthetic/repo-b",
                "attempt": 1,
                "task_summary": "timeout task",
                "critique": "validation_status=failed; error_code=TIMEOUT",
                "reward": 0.1,
                "official_resolved": false,
                "outcome_kind": "validation_failed",
                "tags": ["timeout", "validation-failed", "official-unresolved"]
            }),
            json!({
                "record_kind": "ccreality_autoresearch_attempt",
                "instance_id": "task-c",
                "repo": "synthetic/repo-c",
                "attempt": 2,
                "task_summary": "resolved import task",
                "critique": "validation_status=passed; official resolved",
                "reward": 0.9,
                "official_resolved": true,
                "outcome_kind": "official_resolved",
                "tags": ["import-error", "validation-passed", "official-resolved"]
            }),
        ]
    }

    #[test]
    fn influence_happy_path_writes_audit_and_witness_from_physical_state() {
        let (_tmp, runtime_root, run_id) = seed_runtime();
        write_attempts(&runtime_root, &run_id, &base_rows());
        write_recommendation(&runtime_root, &run_id, "task-a", 1, 1, "import-error");
        write_recommendation(&runtime_root, &run_id, "task-c", 2, 1, "import-error");

        let before = audit_count(&runtime_root, &run_id);
        let result = compute_influence_for_run(
            &runtime_root,
            &run_id,
            &json!({"failure_tag": "import-error", "k": 2, "alpha": 0.35, "tolerance": 1e-12}),
        )
        .expect("influence");
        let after = audit_count(&runtime_root, &run_id);
        println!("HAPPY_PATH_STATE before_audits={before} after_audits={after}");
        assert_eq!(before, 0);
        assert_eq!(after, 1);

        let audit_path = file_arg_to_path(
            result["source_of_truth"]["influence_audit"]
                .as_str()
                .expect("audit path"),
        );
        let audit = read_json(&audit_path).expect("read audit");
        println!(
            "HAPPY_PATH_AUDIT path={} top_attempts={} top_recommendations={} graph_nodes={} graph_edges={}",
            audit_path.display(),
            audit["top_attempts"].as_array().unwrap().len(),
            audit["top_recommendations"].as_array().unwrap().len(),
            audit["graph"]["details"]["node_count"],
            audit["graph"]["details"]["edge_count"]
        );
        assert_eq!(
            audit["record_kind"],
            json!("ccreality_optimizer_influence_computation")
        );
        assert_eq!(audit["failure_tag"], json!("import-error"));
        let ranked_instances = audit["top_attempts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row["instance_id"].as_str().unwrap().to_string())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            ranked_instances,
            BTreeSet::from(["task-a".to_string(), "task-c".to_string()])
        );
        assert!(audit["top_attempts"]
            .as_array()
            .unwrap()
            .iter()
            .all(|row| row["tags"]
                .as_array()
                .unwrap()
                .contains(&json!("import-error"))));

        let witness_path = runtime_root
            .join(&run_id)
            .join("claude-code-optimizer")
            .join("witness-chain.bin");
        let witness_bytes = fs::read(&witness_path).expect("read witness");
        assert_eq!(witness_bytes.len(), WITNESS_ENTRY_SIZE);
        let verification = verify_chain_bytes_with_type_validator(&witness_bytes, |ty| ty == 6)
            .expect("verify witness");
        println!(
            "HAPPY_PATH_WITNESS entries={} chain_hash={}",
            verification.entries,
            hex::encode(verification.last_chain_hash)
        );
        assert_eq!(verification.entries, 1);
        assert_eq!(
            result["influence_audit_sha256"],
            json!(sha256_file(&audit_path).unwrap())
        );
    }

    #[test]
    fn influence_empty_attempts_fails_closed_without_audit() {
        let (_tmp, runtime_root, run_id) = seed_runtime();
        write_text_checked(&attempts_path(&runtime_root, &run_id), "").expect("empty attempts");
        write_recommendation(&runtime_root, &run_id, "task-a", 1, 1, "import-error");
        let before = audit_count(&runtime_root, &run_id);
        let err = compute_influence_for_run(
            &runtime_root,
            &run_id,
            &json!({"failure_tag": "import-error"}),
        )
        .expect_err("empty attempts must fail");
        let after = audit_count(&runtime_root, &run_id);
        println!(
            "EDGE_EMPTY_ATTEMPTS before_audits={before} after_audits={after} error_code={}",
            err.error_code
        );
        assert_eq!(err.error_code, "CCREALITY_INFLUENCE_ATTEMPTS_EMPTY");
        assert_eq!(before, 0);
        assert_eq!(after, 0);
    }

    #[test]
    fn influence_unknown_failure_tag_fails_closed_without_audit() {
        let (_tmp, runtime_root, run_id) = seed_runtime();
        write_attempts(&runtime_root, &run_id, &base_rows());
        write_recommendation(&runtime_root, &run_id, "task-a", 1, 1, "import-error");
        let before = audit_count(&runtime_root, &run_id);
        let err = compute_influence_for_run(
            &runtime_root,
            &run_id,
            &json!({"failure_tag": "syntax-error"}),
        )
        .expect_err("unknown tag must fail");
        let after = audit_count(&runtime_root, &run_id);
        println!(
            "EDGE_UNKNOWN_TAG before_audits={before} after_audits={after} error_code={} details={}",
            err.error_code, err.details
        );
        assert_eq!(err.error_code, "CCREALITY_INFLUENCE_FAILURE_TAG_UNKNOWN");
        assert_eq!(before, 0);
        assert_eq!(after, 0);
    }

    #[test]
    fn influence_orphan_recommendation_fails_closed_without_audit() {
        let (_tmp, runtime_root, run_id) = seed_runtime();
        write_attempts(&runtime_root, &run_id, &base_rows()[..1]);
        write_recommendation(&runtime_root, &run_id, "task-z", 1, 1, "import-error");
        let before = audit_count(&runtime_root, &run_id);
        let err = compute_influence_for_run(
            &runtime_root,
            &run_id,
            &json!({"failure_tag": "import-error"}),
        )
        .expect_err("orphan recommendation must fail");
        let after = audit_count(&runtime_root, &run_id);
        println!(
            "EDGE_ORPHAN_RECOMMENDATION before_audits={before} after_audits={after} error_code={} source_of_truth={:?}",
            err.error_code, err.source_of_truth
        );
        assert_eq!(err.error_code, "CCREALITY_INFLUENCE_ORPHAN_RECOMMENDATION");
        assert_eq!(before, 0);
        assert_eq!(after, 0);
    }

    #[test]
    fn influence_corrupt_witness_preflight_fails_before_audit_write() {
        let (_tmp, runtime_root, run_id) = seed_runtime();
        write_attempts(&runtime_root, &run_id, &base_rows());
        write_recommendation(&runtime_root, &run_id, "task-a", 1, 1, "import-error");
        write_recommendation(&runtime_root, &run_id, "task-c", 2, 1, "import-error");
        let witness_path = write_corrupt_witness_chain(&runtime_root, &run_id);
        let before_audits = audit_count(&runtime_root, &run_id);
        let before_witness_sha = sha256_file(&witness_path).expect("before witness sha");
        let err = compute_influence_for_run(
            &runtime_root,
            &run_id,
            &json!({"failure_tag": "import-error", "k": 2}),
        )
        .expect_err("corrupt witness must fail before audit");
        let after_audits = audit_count(&runtime_root, &run_id);
        let after_witness_sha = sha256_file(&witness_path).expect("after witness sha");
        println!(
            "EDGE_CORRUPT_WITNESS_PREFLIGHT before_audits={before_audits} after_audits={after_audits} before_witness_sha={before_witness_sha} after_witness_sha={after_witness_sha} error_code={}",
            err.error_code
        );
        assert_eq!(err.error_code, "CCREALITY_WITNESS_PREV_HASH_MISMATCH");
        assert_eq!(before_audits, 0);
        assert_eq!(after_audits, 0);
        assert_eq!(before_witness_sha, after_witness_sha);
    }
}
