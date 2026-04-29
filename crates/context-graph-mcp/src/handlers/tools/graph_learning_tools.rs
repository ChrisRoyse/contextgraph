//! Graph-learning MCP handlers and policy resolver.
//!
//! This layer never mutates raw graph edges. It records observed graph outcomes
//! as `LearningEvent` rows in `CF_LEARNING_EVENTS`, then resolves an explicit
//! learned ranking policy from those persisted rows.

use std::collections::{BTreeMap, HashMap, HashSet};

use chrono::Utc;
use context_graph_core::learning::{
    LearningEvent, LearningOutcome, LearningOutcomeLabel, LearningStateSnapshot,
};
use context_graph_core::training::NUM_CROSS_CORRELATIONS;
use context_graph_core::types::fingerprint::NUM_EMBEDDERS;
use context_graph_core::weights::{get_effective_weight_profile, validate_weights};
use context_graph_storage::teleological::RocksDbTeleologicalStore;
use context_graph_storage::{EdgeRepository, GraphEdgeStats};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};
use tracing::{error, info};
use uuid::Uuid;

use crate::handlers::Handlers;
use crate::protocol::{JsonRpcId, JsonRpcResponse};

use super::graph_link_dtos::{embedder_name, SEMANTIC_EMBEDDER_INDICES};

const GRAPH_LEARNING_TASK_PREFIX: &str = "graph_learning:";
const GRAPH_LEARNING_CONTEXT_VERSION: u8 = 1;
const DEFAULT_GRAPH_LEARNING_MAX_SCAN: usize = 10_000;
const DEFAULT_GRAPH_LEARNING_RATE: f32 = 0.35;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RecordGraphLearningEventArgs {
    event_id: Option<Uuid>,
    policy_scope: String,
    #[serde(default = "default_graph_tool")]
    graph_tool: String,
    source_memory_id: Uuid,
    selected_neighbor_ids: Vec<Uuid>,
    #[serde(default)]
    rejected_neighbor_ids: Vec<Uuid>,
    #[serde(default = "default_weight_profile")]
    weight_profile: String,
    before_rank: u32,
    after_rank: u32,
    #[serde(default)]
    query: String,
    session_id: Option<String>,
    response_id: Option<String>,
    outcome: GraphLearningOutcomeArgs,
    outcome_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResolveGraphLearningPolicyArgs {
    policy_scope: String,
    source_memory_id: Option<Uuid>,
    #[serde(default = "default_weight_profile")]
    base_weight_profile: String,
    #[serde(default = "default_min_evidence")]
    min_evidence: usize,
    #[serde(default = "default_graph_learning_max_scan")]
    max_scan: usize,
    #[serde(default = "default_graph_learning_rate")]
    learning_rate: f32,
    #[serde(default = "default_true")]
    include_evidence: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphLearningOutcomeArgs {
    label: String,
    utility_delta: f32,
    #[serde(default)]
    correction_required: bool,
    #[serde(default)]
    reuse_observed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct GraphLearningContext {
    version: u8,
    policy_scope: String,
    graph_tool: String,
    source_memory_id: Uuid,
    selected_neighbor_ids: Vec<Uuid>,
    rejected_neighbor_ids: Vec<Uuid>,
    weight_profile: String,
    before_rank: u32,
    after_rank: u32,
    selected_edges: Vec<GraphLearningEdgeEvidence>,
    rejected_edges: Vec<GraphLearningEdgeEvidence>,
    edge_stats: GraphLearningEdgeStats,
    observed_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct GraphLearningEdgeEvidence {
    target_id: Uuid,
    rank: u32,
    normalized_embedder_scores: [f32; NUM_EMBEDDERS],
    observed_embedders: Vec<usize>,
    mean_score: f32,
    typed_edge_weight: Option<f32>,
    typed_edge_type: Option<String>,
    typed_edge_agreement_count: Option<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GraphLearningEdgeStats {
    embedder_edge_rows: u64,
    typed_edge_count: u64,
}

#[derive(Debug, Clone)]
pub(crate) struct ResolvedGraphLearningPolicy {
    pub policy_scope: String,
    pub source_memory_id: Option<Uuid>,
    pub base_weights: [f32; NUM_EMBEDDERS],
    pub learned_weights: [f32; NUM_EMBEDDERS],
    pub edge_boosts: HashMap<Uuid, f32>,
    pub scanned_events: usize,
    pub matched_events: usize,
    pub evidence_events: usize,
    pub learning_rate: f32,
    pub evidence: Vec<JsonValue>,
}

#[derive(Debug, Clone)]
pub(crate) struct GraphLearningPolicyResolveParams {
    pub source_memory_id: Option<Uuid>,
    pub policy_scope: String,
    pub base_weights: [f32; NUM_EMBEDDERS],
    pub min_evidence: usize,
    pub max_scan: usize,
    pub learning_rate: f32,
    pub include_evidence: bool,
}

impl ResolvedGraphLearningPolicy {
    pub(crate) fn apply_score(&self, target_id: Uuid, base_score: f32) -> (f32, f32) {
        let boost = self.edge_boosts.get(&target_id).copied().unwrap_or(0.0);
        ((base_score + boost).max(0.0), boost)
    }

    pub(crate) fn metadata(&self, include_edge_boosts: bool) -> JsonValue {
        let mut edge_boosts = self
            .edge_boosts
            .iter()
            .map(|(target_id, boost)| {
                json!({
                    "target_id": target_id.to_string(),
                    "additive_rrf_boost": boost,
                })
            })
            .collect::<Vec<_>>();
        edge_boosts.sort_by(|a, b| a["target_id"].as_str().cmp(&b["target_id"].as_str()));

        let mut value = json!({
            "policy_scope": self.policy_scope,
            "source_memory_id": self.source_memory_id.map(|id| id.to_string()),
            "source_of_truth": {
                "backend": "rocksdb",
                "column_family": "learning_events",
                "task_id_prefix": GRAPH_LEARNING_TASK_PREFIX,
            },
            "scanned_events": self.scanned_events,
            "matched_events": self.matched_events,
            "evidence_events": self.evidence_events,
            "learning_rate": self.learning_rate,
            "base_weights": render_weight_vector(&self.base_weights),
            "learned_weights": render_weight_vector(&self.learned_weights),
            "edge_boost_count": self.edge_boosts.len(),
        });
        if include_edge_boosts {
            value["edge_boosts"] = json!(edge_boosts);
        }
        if !self.evidence.is_empty() {
            value["evidence"] = json!(self.evidence);
        }
        value
    }
}

impl TryFrom<GraphLearningOutcomeArgs> for LearningOutcome {
    type Error = String;

    fn try_from(value: GraphLearningOutcomeArgs) -> Result<Self, Self::Error> {
        if !value.utility_delta.is_finite() || !(-1.0..=1.0).contains(&value.utility_delta) {
            return Err("outcome.utilityDelta must be finite and in [-1, 1]".into());
        }
        let label = match value.label.as_str() {
            "useful" => LearningOutcomeLabel::Useful,
            "neutral" => LearningOutcomeLabel::Neutral,
            "harmful" => LearningOutcomeLabel::Harmful,
            "no_learning" => LearningOutcomeLabel::NoLearning,
            other => {
                return Err(format!(
                    "outcome.label must be useful, neutral, harmful, or no_learning; got {other}"
                ));
            }
        };
        Ok(Self {
            label,
            utility_delta: value.utility_delta,
            correction_required: value.correction_required,
            reuse_observed: value.reuse_observed,
        })
    }
}

impl Handlers {
    pub(crate) async fn call_record_graph_learning_event(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let parsed: RecordGraphLearningEventArgs = match serde_json::from_value(args) {
            Ok(v) => v,
            Err(e) => return self.tool_error(id, &format!("Invalid graph learning args: {e}")),
        };
        if let Err(e) = validate_policy_scope(&parsed.policy_scope) {
            return self.tool_error(id, &e);
        }
        if parsed.selected_neighbor_ids.is_empty() {
            return self.tool_error(id, "selectedNeighborIds must contain at least one UUID");
        }
        if parsed.before_rank == 0 || parsed.after_rank == 0 {
            return self.tool_error(id, "beforeRank and afterRank must be >= 1");
        }
        if !matches!(
            parsed.graph_tool.as_str(),
            "get_unified_neighbors" | "get_memory_neighbors" | "traverse_graph"
        ) {
            return self.tool_error(
                id,
                "graphTool must be get_unified_neighbors, get_memory_neighbors, or traverse_graph",
            );
        }
        let outcome = match LearningOutcome::try_from(parsed.outcome.clone()) {
            Ok(v) => v,
            Err(e) => return self.tool_error(id, &e),
        };

        let Some(rocksdb_store) = self
            .teleological_store
            .as_any()
            .downcast_ref::<RocksDbTeleologicalStore>()
        else {
            return self.tool_error(
                id,
                "record_graph_learning_event requires RocksDbTeleologicalStore.",
            );
        };
        let Some(edge_repo) = self.edge_repository.as_ref() else {
            error!("record_graph_learning_event: EdgeRepository is not initialized");
            return self.tool_error(
                id,
                "record_graph_learning_event requires EdgeRepository; graph linking is not initialized.",
            );
        };

        let before_count = match rocksdb_store.count_learning_events().await {
            Ok(v) => v,
            Err(e) => return self.tool_error(id, &format!("count_learning_events failed: {e}")),
        };

        let mut memory_ids = vec![parsed.source_memory_id];
        memory_ids.extend(parsed.selected_neighbor_ids.iter().copied());
        memory_ids.extend(parsed.rejected_neighbor_ids.iter().copied());
        dedupe_uuids(&mut memory_ids);
        if let Err(e) = self.verify_graph_learning_memories(&memory_ids).await {
            return self.tool_error(id, &e);
        }

        let selected_edges = match collect_edge_evidence_set(
            edge_repo,
            parsed.source_memory_id,
            &parsed.selected_neighbor_ids,
        ) {
            Ok(v) => v,
            Err(e) => return self.tool_error(id, &e),
        };
        let rejected_edges = match collect_edge_evidence_set(
            edge_repo,
            parsed.source_memory_id,
            &parsed.rejected_neighbor_ids,
        ) {
            Ok(v) => v,
            Err(e) => return self.tool_error(id, &e),
        };
        let edge_stats = match edge_repo.get_stats() {
            Ok(v) => render_edge_stats_for_context(&v),
            Err(e) => return self.tool_error(id, &format!("EdgeRepository get_stats failed: {e}")),
        };
        if edge_stats.embedder_edge_rows == 0 {
            return self.tool_error(
                id,
                "EdgeRepository has zero embedder_edges rows; cannot record graph learning without graph evidence",
            );
        }

        let context = GraphLearningContext {
            version: GRAPH_LEARNING_CONTEXT_VERSION,
            policy_scope: parsed.policy_scope.clone(),
            graph_tool: parsed.graph_tool.clone(),
            source_memory_id: parsed.source_memory_id,
            selected_neighbor_ids: parsed.selected_neighbor_ids.clone(),
            rejected_neighbor_ids: parsed.rejected_neighbor_ids.clone(),
            weight_profile: parsed.weight_profile.clone(),
            before_rank: parsed.before_rank,
            after_rank: parsed.after_rank,
            selected_edges,
            rejected_edges,
            edge_stats,
            observed_at: Utc::now().to_rfc3339(),
        };
        if let Err(e) = validate_graph_learning_context(&context) {
            error!(error = %e, "record_graph_learning_event: context validation failed");
            return self.tool_error(
                id,
                &format!("derived graph learning context failed validation: {e}"),
            );
        }

        let (before, after) = match build_learning_states_from_graph_context(&context, &outcome) {
            Ok(v) => v,
            Err(e) => return self.tool_error(id, &e),
        };
        let retrieved_context = match serde_json::to_string(&context) {
            Ok(v) => v,
            Err(e) => return self.tool_error(id, &format!("serialize graph context failed: {e}")),
        };
        let assistant_response = parsed.outcome_reason.unwrap_or_default();
        let event_id = parsed.event_id.unwrap_or_else(Uuid::new_v4);
        let task_id = graph_learning_task_id(&parsed.policy_scope);

        let event = match LearningEvent::new(
            event_id,
            memory_ids,
            parsed.session_id,
            parsed.response_id,
            Some(task_id.clone()),
            parsed.query,
            retrieved_context,
            assistant_response,
            before,
            after,
            outcome,
        ) {
            Ok(v) => v,
            Err(e) => {
                error!(event_id = %event_id, error = %e, "record_graph_learning_event: LearningEvent validation failed");
                return self.tool_error(id, &format!("LearningEvent validation failed: {e}"));
            }
        };
        if let Err(e) = rocksdb_store.store_learning_event(&event).await {
            error!(event_id = %event.event_id, error = %e, "record_graph_learning_event: store_learning_event failed");
            return self.tool_error(id, &format!("store_learning_event failed: {e}"));
        }

        let after_count = match rocksdb_store.count_learning_events().await {
            Ok(v) => v,
            Err(e) => {
                return self
                    .tool_error(id, &format!("post-store count_learning_events failed: {e}"))
            }
        };
        let readback = match rocksdb_store.get_learning_event(event.event_id).await {
            Ok(Some(v)) => v,
            Ok(None) => {
                return self.tool_error(
                    id,
                    &format!(
                        "post-store readback failed: event {} missing from CF_LEARNING_EVENTS",
                        event.event_id
                    ),
                )
            }
            Err(e) => return self.tool_error(id, &format!("post-store readback failed: {e}")),
        };
        let readback_context = match parse_graph_learning_context(&readback) {
            Ok(v) => v,
            Err(e) => return self.tool_error(id, &format!("readback context invalid: {e}")),
        };
        if readback.task_id.as_deref() != Some(task_id.as_str())
            || readback_context.source_memory_id != parsed.source_memory_id
            || readback_context.policy_scope != parsed.policy_scope
        {
            return self.tool_error(
                id,
                "post-store readback mismatch: persisted graph learning event does not match request",
            );
        }

        info!(
            event_id = %event.event_id,
            policy_scope = %parsed.policy_scope,
            source_memory_id = %parsed.source_memory_id,
            before_count = before_count,
            after_count = after_count,
            "record_graph_learning_event: stored and verified graph LearningEvent"
        );

        self.tool_result(
            id,
            json!({
                "status": "stored",
                "event_id": event.event_id.to_string(),
                "source_of_truth": {
                    "backend": "rocksdb",
                    "column_family": "learning_events",
                    "format": "version_byte + bincode",
                    "task_id": task_id,
                },
                "before_count": before_count,
                "after_count": after_count,
                "readback_verified": true,
                "graph_evidence": render_graph_context_summary(&readback_context),
                "features": {
                    "delta_e_scalar": readback.features.delta_e_scalar,
                    "retrieval_rank_shift": readback.features.retrieval_rank_shift,
                    "embedder_disagreement": readback.features.embedder_disagreement,
                    "coherence_delta": readback.features.coherence_delta,
                    "multi_utl_score": readback.features.multi_utl_score,
                }
            }),
        )
    }

    pub(crate) async fn call_resolve_graph_learning_policy(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let parsed: ResolveGraphLearningPolicyArgs = match serde_json::from_value(args) {
            Ok(v) => v,
            Err(e) => {
                return self.tool_error(id, &format!("Invalid graph policy resolve args: {e}"))
            }
        };
        let base_weights = match get_effective_weight_profile(&parsed.base_weight_profile) {
            Ok(v) => v,
            Err(e) => return self.tool_error(id, &format!("Invalid baseWeightProfile: {e}")),
        };
        let Some(rocksdb_store) = self
            .teleological_store
            .as_any()
            .downcast_ref::<RocksDbTeleologicalStore>()
        else {
            return self.tool_error(
                id,
                "resolve_graph_learning_policy requires RocksDbTeleologicalStore.",
            );
        };
        let policy = match resolve_graph_learning_policy(
            rocksdb_store,
            GraphLearningPolicyResolveParams {
                source_memory_id: parsed.source_memory_id,
                policy_scope: parsed.policy_scope,
                base_weights,
                min_evidence: parsed.min_evidence,
                max_scan: parsed.max_scan,
                learning_rate: parsed.learning_rate,
                include_evidence: parsed.include_evidence,
            },
        )
        .await
        {
            Ok(v) => v,
            Err(e) => return self.tool_error(id, &e),
        };
        self.tool_result(id, policy.metadata(true))
    }

    async fn verify_graph_learning_memories(&self, memory_ids: &[Uuid]) -> Result<(), String> {
        for memory_id in memory_ids {
            match self.teleological_store.retrieve(*memory_id).await {
                Ok(Some(_)) => {}
                Ok(None) => return Err(format!("memory {memory_id} is missing from fingerprints")),
                Err(e) => return Err(format!("failed to read fingerprint {memory_id}: {e}")),
            }
            match self.teleological_store.get_content(*memory_id).await {
                Ok(Some(content)) if !content.is_empty() => {}
                Ok(Some(_)) => return Err(format!("memory {memory_id} has empty content row")),
                Ok(None) => return Err(format!("memory {memory_id} is missing content row")),
                Err(e) => return Err(format!("failed to read content {memory_id}: {e}")),
            }
        }
        Ok(())
    }
}

pub(crate) async fn resolve_graph_learning_policy(
    rocksdb_store: &RocksDbTeleologicalStore,
    params: GraphLearningPolicyResolveParams,
) -> Result<ResolvedGraphLearningPolicy, String> {
    let policy_scope = params.policy_scope.as_str();
    let source_memory_id = params.source_memory_id;
    let base_weights = params.base_weights;
    let min_evidence = params.min_evidence;
    let max_scan = params.max_scan;
    let learning_rate = params.learning_rate;
    let include_evidence = params.include_evidence;

    validate_policy_scope(policy_scope)?;
    if min_evidence == 0 || min_evidence > 10_000 {
        return Err("minEvidence must be in [1, 10000]".into());
    }
    if max_scan == 0 || max_scan > 200_000 {
        return Err("maxScan must be in [1, 200000]".into());
    }
    if !learning_rate.is_finite() || !(0.0..=1.0).contains(&learning_rate) {
        return Err("learningRate must be finite and in [0, 1]".into());
    }
    validate_weights(&base_weights).map_err(|e| format!("base weights invalid: {e}"))?;

    let before_count = rocksdb_store
        .count_learning_events()
        .await
        .map_err(|e| format!("count_learning_events failed: {e}"))?;
    if before_count == 0 {
        return Err("CF_LEARNING_EVENTS is empty; graph learning policy has no evidence".into());
    }
    let ids = rocksdb_store
        .list_learning_event_ids()
        .await
        .map_err(|e| format!("list_learning_event_ids failed: {e}"))?;

    let task_id = graph_learning_task_id(policy_scope);
    let mut scanned_events = 0usize;
    let mut matched_events = 0usize;
    let mut evidence_events = 0usize;
    let mut utility_by_embedder = [0.0f32; NUM_EMBEDDERS];
    let mut evidence_weight_by_embedder = [0.0f32; NUM_EMBEDDERS];
    let mut edge_boost_sums: HashMap<Uuid, (f32, usize)> = HashMap::new();
    let mut evidence = Vec::new();

    for event_id in ids.into_iter().take(max_scan) {
        let event = match rocksdb_store.get_learning_event(event_id).await {
            Ok(Some(v)) => v,
            Ok(None) => {
                return Err(format!(
                    "list_learning_event_ids returned {event_id}, but point read was missing"
                ))
            }
            Err(e) => return Err(format!("get_learning_event failed for {event_id}: {e}")),
        };
        scanned_events += 1;
        if event.task_id.as_deref() != Some(task_id.as_str()) {
            continue;
        }
        let context = parse_graph_learning_context(&event)?;
        if let Some(source_filter) = source_memory_id {
            if context.source_memory_id != source_filter {
                continue;
            }
        }
        matched_events += 1;
        validate_graph_learning_context(&context)?;
        if context.selected_edges.is_empty() {
            return Err(format!(
                "graph learning event {} has no selected edge evidence",
                event.event_id
            ));
        }

        let utility = event.outcome.utility_delta;
        for edge in &context.selected_edges {
            for idx in 0..NUM_EMBEDDERS {
                let score = edge.normalized_embedder_scores[idx];
                if score > 0.0 {
                    utility_by_embedder[idx] += utility * score;
                    evidence_weight_by_embedder[idx] += score;
                }
            }
            let boost_signal = utility * edge.mean_score;
            let entry = edge_boost_sums.entry(edge.target_id).or_insert((0.0, 0));
            entry.0 += boost_signal;
            entry.1 += 1;
        }
        for edge in &context.rejected_edges {
            let reject_signal = -utility.abs() * edge.mean_score;
            let entry = edge_boost_sums.entry(edge.target_id).or_insert((0.0, 0));
            entry.0 += reject_signal;
            entry.1 += 1;
        }
        evidence_events += 1;
        if include_evidence {
            evidence.push(json!({
                "event_id": event.event_id.to_string(),
                "utility_delta": event.outcome.utility_delta,
                "outcome_label": outcome_label_str(event.outcome.label),
                "source_memory_id": context.source_memory_id.to_string(),
                "selected_neighbor_ids": context.selected_neighbor_ids.iter().map(|id| id.to_string()).collect::<Vec<_>>(),
                "rejected_neighbor_ids": context.rejected_neighbor_ids.iter().map(|id| id.to_string()).collect::<Vec<_>>(),
                "before_rank": context.before_rank,
                "after_rank": context.after_rank,
            }));
        }
    }

    if evidence_events < min_evidence {
        return Err(format!(
            "insufficient graph learning evidence for scope '{}': matched {}, required {}",
            policy_scope, evidence_events, min_evidence
        ));
    }

    let mut learned_weights = base_weights;
    for idx in 0..NUM_EMBEDDERS {
        if evidence_weight_by_embedder[idx] > 0.0 {
            let avg_utility = utility_by_embedder[idx] / evidence_weight_by_embedder[idx];
            learned_weights[idx] =
                (base_weights[idx] * (1.0 + learning_rate * avg_utility)).max(0.0);
        }
    }
    normalize_weights(&mut learned_weights)?;

    let edge_boosts = edge_boost_sums
        .into_iter()
        .map(|(target_id, (sum, count))| {
            let avg = if count == 0 { 0.0 } else { sum / count as f32 };
            (target_id, (avg * learning_rate).clamp(-0.25, 0.25))
        })
        .filter(|(_, boost)| boost.abs() > f32::EPSILON)
        .collect::<HashMap<_, _>>();

    Ok(ResolvedGraphLearningPolicy {
        policy_scope: policy_scope.to_string(),
        source_memory_id,
        base_weights,
        learned_weights,
        edge_boosts,
        scanned_events,
        matched_events,
        evidence_events,
        learning_rate,
        evidence,
    })
}

fn collect_edge_evidence_set(
    edge_repo: &EdgeRepository,
    source: Uuid,
    target_ids: &[Uuid],
) -> Result<Vec<GraphLearningEdgeEvidence>, String> {
    let mut out = Vec::with_capacity(target_ids.len());
    for (rank, target_id) in target_ids.iter().enumerate() {
        if *target_id == source {
            return Err("graph learning target cannot equal sourceMemoryId".into());
        }
        out.push(collect_edge_evidence(
            edge_repo,
            source,
            *target_id,
            (rank + 1) as u32,
        )?);
    }
    Ok(out)
}

fn collect_edge_evidence(
    edge_repo: &EdgeRepository,
    source: Uuid,
    target: Uuid,
    rank: u32,
) -> Result<GraphLearningEdgeEvidence, String> {
    let mut normalized_embedder_scores = [0.0f32; NUM_EMBEDDERS];
    let mut observed_embedders = Vec::new();
    for &embedder_idx in &SEMANTIC_EMBEDDER_INDICES {
        let edges = edge_repo
            .get_embedder_edges(embedder_idx as u8, source)
            .map_err(|e| {
                format!(
                    "failed to read embedder_edges for source {source}, embedder {embedder_idx}: {e}"
                )
            })?;
        if let Some(edge) = edges.iter().find(|edge| edge.target() == target) {
            let raw = edge.similarity();
            if !raw.is_finite() || !(-1.0..=1.0).contains(&raw) {
                return Err(format!(
                    "edge {source}->{target} embedder {embedder_idx} has invalid similarity {raw}"
                ));
            }
            let normalized = (raw + 1.0) / 2.0;
            if !normalized.is_finite() || !(0.0..=1.0).contains(&normalized) {
                return Err(format!(
                    "edge {source}->{target} embedder {embedder_idx} normalized similarity was invalid: {normalized}"
                ));
            }
            normalized_embedder_scores[embedder_idx] = normalized;
            observed_embedders.push(embedder_idx);
        }
    }
    if observed_embedders.is_empty() {
        return Err(format!(
            "no K-NN graph evidence exists for source {source} -> target {target}; run graph building first"
        ));
    }
    let mean_score = observed_embedders
        .iter()
        .map(|idx| normalized_embedder_scores[*idx])
        .sum::<f32>()
        / observed_embedders.len() as f32;
    let typed_edge = edge_repo
        .get_typed_edge(source, target)
        .map_err(|e| format!("failed to read typed edge {source}->{target}: {e}"))?;
    Ok(GraphLearningEdgeEvidence {
        target_id: target,
        rank,
        normalized_embedder_scores,
        observed_embedders,
        mean_score,
        typed_edge_weight: typed_edge.as_ref().map(|edge| edge.weight()),
        typed_edge_type: typed_edge
            .as_ref()
            .map(|edge| format!("{:?}", edge.edge_type())),
        typed_edge_agreement_count: typed_edge.as_ref().map(|edge| edge.agreement_count()),
    })
}

fn build_learning_states_from_graph_context(
    context: &GraphLearningContext,
    outcome: &LearningOutcome,
) -> Result<(LearningStateSnapshot, LearningStateSnapshot), String> {
    let before_profile = average_edge_scores(&context.selected_edges)?;
    let after_profile = apply_outcome_to_profile(before_profile, outcome.utility_delta)?;
    let rejected_pressure = if context.rejected_edges.is_empty() {
        0.0
    } else {
        context
            .rejected_edges
            .iter()
            .map(|edge| edge.mean_score)
            .sum::<f32>()
            / context.rejected_edges.len() as f32
    };
    let selected_mean = context
        .selected_edges
        .iter()
        .map(|edge| edge.mean_score)
        .sum::<f32>()
        / context.selected_edges.len() as f32;
    let after_contradiction = if outcome.correction_required || outcome.utility_delta < 0.0 {
        (rejected_pressure + outcome.utility_delta.abs() * 0.5).min(1.0)
    } else {
        (rejected_pressure * (1.0 - outcome.utility_delta.max(0.0))).max(0.0)
    };
    let after_integration =
        (selected_mean + outcome.utility_delta.max(0.0) * (1.0 - selected_mean)).clamp(0.0, 1.0);
    let domain = Some(format!("graph:{}", context.policy_scope));

    let before = LearningStateSnapshot {
        topic_profile: before_profile,
        cross_correlations: cross_correlations_from_profile(&before_profile),
        retrieval_rank: Some(context.before_rank),
        embedder_scores: before_profile,
        contradiction_pressure: rejected_pressure.clamp(0.0, 1.0),
        integration_confidence: selected_mean.clamp(0.0, 1.0),
        recurrence_count: context.selected_edges.len() as u32,
        stability_score: selected_mean.clamp(0.0, 1.0),
        domain: domain.clone(),
        successful_transfer_count: 0,
    };
    let after = LearningStateSnapshot {
        topic_profile: after_profile,
        cross_correlations: cross_correlations_from_profile(&after_profile),
        retrieval_rank: Some(context.after_rank),
        embedder_scores: after_profile,
        contradiction_pressure: after_contradiction,
        integration_confidence: after_integration,
        recurrence_count: context.selected_edges.len() as u32 + u32::from(outcome.reuse_observed),
        stability_score: after_integration,
        domain,
        successful_transfer_count: u32::from(outcome.reuse_observed),
    };
    before
        .validate("graph_learning.before")
        .map_err(|e| format!("graph learning before-state validation failed: {e}"))?;
    after
        .validate("graph_learning.after")
        .map_err(|e| format!("graph learning after-state validation failed: {e}"))?;
    Ok((before, after))
}

fn average_edge_scores(
    edges: &[GraphLearningEdgeEvidence],
) -> Result<[f32; NUM_EMBEDDERS], String> {
    if edges.is_empty() {
        return Err("cannot average empty graph learning edge evidence".into());
    }
    let mut sums = [0.0f32; NUM_EMBEDDERS];
    let mut counts = [0usize; NUM_EMBEDDERS];
    for edge in edges {
        for idx in 0..NUM_EMBEDDERS {
            let score = edge.normalized_embedder_scores[idx];
            if score > 0.0 {
                sums[idx] += score;
                counts[idx] += 1;
            }
        }
    }
    let mut out = [0.0f32; NUM_EMBEDDERS];
    for idx in 0..NUM_EMBEDDERS {
        out[idx] = if counts[idx] > 0 {
            sums[idx] / counts[idx] as f32
        } else {
            0.0
        };
    }
    Ok(out)
}

fn apply_outcome_to_profile(
    before: [f32; NUM_EMBEDDERS],
    utility_delta: f32,
) -> Result<[f32; NUM_EMBEDDERS], String> {
    if !utility_delta.is_finite() || !(-1.0..=1.0).contains(&utility_delta) {
        return Err("utility delta must be finite and in [-1, 1]".into());
    }
    let mut after = [0.0f32; NUM_EMBEDDERS];
    for idx in 0..NUM_EMBEDDERS {
        let value = before[idx];
        after[idx] = if utility_delta >= 0.0 {
            value + (1.0 - value) * utility_delta
        } else {
            value * (1.0 + utility_delta)
        };
        if !after[idx].is_finite() || !(0.0..=1.0).contains(&after[idx]) {
            return Err(format!(
                "derived after profile for embedder {idx} was invalid: {}",
                after[idx]
            ));
        }
    }
    Ok(after)
}

fn cross_correlations_from_profile(profile: &[f32; NUM_EMBEDDERS]) -> Vec<f32> {
    let mut out = Vec::with_capacity(NUM_CROSS_CORRELATIONS);
    for i in 0..NUM_EMBEDDERS {
        for j in (i + 1)..NUM_EMBEDDERS {
            out.push((profile[i] * profile[j]).clamp(0.0, 1.0));
        }
    }
    out
}

fn parse_graph_learning_context(event: &LearningEvent) -> Result<GraphLearningContext, String> {
    let context: GraphLearningContext =
        serde_json::from_str(&event.retrieved_context).map_err(|e| {
            format!(
                "event {} retrieved_context is not graph context JSON: {e}",
                event.event_id
            )
        })?;
    validate_graph_learning_context(&context)?;
    Ok(context)
}

fn validate_graph_learning_context(context: &GraphLearningContext) -> Result<(), String> {
    if context.version != GRAPH_LEARNING_CONTEXT_VERSION {
        return Err(format!(
            "graph learning context version mismatch: got {}, expected {}",
            context.version, GRAPH_LEARNING_CONTEXT_VERSION
        ));
    }
    validate_policy_scope(&context.policy_scope)?;
    if context.selected_edges.is_empty() {
        return Err("graph learning context selected_edges must not be empty".into());
    }
    for edge in context
        .selected_edges
        .iter()
        .chain(context.rejected_edges.iter())
    {
        if edge.observed_embedders.is_empty() {
            return Err(format!("edge {} has no observed embedders", edge.target_id));
        }
        if !edge.mean_score.is_finite() || !(0.0..=1.0).contains(&edge.mean_score) {
            return Err(format!(
                "edge {} mean_score invalid: {}",
                edge.target_id, edge.mean_score
            ));
        }
        for idx in 0..NUM_EMBEDDERS {
            let score = edge.normalized_embedder_scores[idx];
            if !score.is_finite() || !(0.0..=1.0).contains(&score) {
                return Err(format!(
                    "edge {} normalized_embedder_scores[{idx}] invalid: {score}",
                    edge.target_id
                ));
            }
        }
    }
    Ok(())
}

fn render_edge_stats_for_context(stats: &GraphEdgeStats) -> GraphLearningEdgeStats {
    GraphLearningEdgeStats {
        embedder_edge_rows: stats.total_embedder_edges,
        typed_edge_count: stats.typed_edge_count,
    }
}

fn render_graph_context_summary(context: &GraphLearningContext) -> JsonValue {
    json!({
        "version": context.version,
        "policy_scope": context.policy_scope,
        "graph_tool": context.graph_tool,
        "source_memory_id": context.source_memory_id.to_string(),
        "selected_neighbor_ids": context.selected_neighbor_ids.iter().map(|id| id.to_string()).collect::<Vec<_>>(),
        "rejected_neighbor_ids": context.rejected_neighbor_ids.iter().map(|id| id.to_string()).collect::<Vec<_>>(),
        "selected_edges": context.selected_edges.iter().map(render_edge_evidence).collect::<Vec<_>>(),
        "rejected_edges": context.rejected_edges.iter().map(render_edge_evidence).collect::<Vec<_>>(),
        "edge_stats": {
            "embedder_edge_rows": context.edge_stats.embedder_edge_rows,
            "typed_edge_count": context.edge_stats.typed_edge_count,
        }
    })
}

fn render_edge_evidence(edge: &GraphLearningEdgeEvidence) -> JsonValue {
    json!({
        "target_id": edge.target_id.to_string(),
        "rank": edge.rank,
        "mean_score": edge.mean_score,
        "observed_embedders": edge.observed_embedders.iter().map(|idx| json!({
            "index": idx,
            "name": embedder_name(*idx),
            "score": edge.normalized_embedder_scores[*idx],
        })).collect::<Vec<_>>(),
        "typed_edge_weight": edge.typed_edge_weight,
        "typed_edge_type": edge.typed_edge_type,
        "typed_edge_agreement_count": edge.typed_edge_agreement_count,
    })
}

fn render_weight_vector(weights: &[f32; NUM_EMBEDDERS]) -> JsonValue {
    let mut map = BTreeMap::new();
    for (idx, weight) in weights.iter().enumerate() {
        map.insert(format!("E{}", idx + 1), *weight);
    }
    json!(map)
}

fn normalize_weights(weights: &mut [f32; NUM_EMBEDDERS]) -> Result<(), String> {
    for (idx, value) in weights.iter().enumerate() {
        if !value.is_finite() || *value < 0.0 {
            return Err(format!("learned weight index {idx} invalid: {value}"));
        }
    }
    let sum = weights.iter().sum::<f32>();
    if !sum.is_finite() || sum <= 0.0 {
        return Err("learned graph policy produced zero total weight".into());
    }
    for value in weights.iter_mut() {
        *value /= sum;
    }
    validate_weights(weights).map_err(|e| format!("learned weights invalid after normalize: {e}"))
}

fn graph_learning_task_id(scope: &str) -> String {
    format!("{GRAPH_LEARNING_TASK_PREFIX}{scope}")
}

fn validate_policy_scope(scope: &str) -> Result<(), String> {
    if scope.trim().is_empty() {
        return Err("policyScope must not be empty".into());
    }
    if scope.chars().count() > 128 {
        return Err("policyScope must be <= 128 characters".into());
    }
    if scope.contains(char::is_whitespace) {
        return Err("policyScope must not contain whitespace".into());
    }
    Ok(())
}

fn outcome_label_str(label: LearningOutcomeLabel) -> &'static str {
    match label {
        LearningOutcomeLabel::Useful => "useful",
        LearningOutcomeLabel::Neutral => "neutral",
        LearningOutcomeLabel::Harmful => "harmful",
        LearningOutcomeLabel::NoLearning => "no_learning",
    }
}

fn dedupe_uuids(values: &mut Vec<Uuid>) {
    let mut seen = HashSet::new();
    values.retain(|id| seen.insert(*id));
}

fn default_graph_tool() -> String {
    "get_unified_neighbors".into()
}

fn default_weight_profile() -> String {
    "semantic_search".into()
}

fn default_min_evidence() -> usize {
    1
}

fn default_graph_learning_max_scan() -> usize {
    DEFAULT_GRAPH_LEARNING_MAX_SCAN
}

fn default_graph_learning_rate() -> f32 {
    DEFAULT_GRAPH_LEARNING_RATE
}

fn default_true() -> bool {
    true
}
