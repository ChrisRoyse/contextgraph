//! Learning-as-UTL event tool handlers.

use std::collections::BTreeMap;

use context_graph_core::learner::{sha256_json, LearnerModality};
use context_graph_core::learner_training::{
    learning_event_feature_schema, learning_event_feature_vector, learning_event_label_schema,
    LearnerTrainingDataset, LearnerTrainingRow, LearnerTrainingTask,
};
use context_graph_core::learning::{
    DeterministicLearningSignalEmbedder, LearningEvent, LearningOutcome, LearningOutcomeLabel,
    LearningSignal, LearningSignalEmbedder, LearningSignalId, LearningStateSnapshot,
};
use context_graph_core::types::fingerprint::NUM_EMBEDDERS;
use context_graph_storage::teleological::RocksDbTeleologicalStore;
use serde::Deserialize;
use serde_json::json;
use tracing::error;
use uuid::Uuid;

use crate::handlers::Handlers;
use crate::protocol::{JsonRpcId, JsonRpcResponse};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RecordLearningEventArgs {
    event_id: Option<Uuid>,
    #[serde(default)]
    memory_ids: Vec<Uuid>,
    session_id: Option<String>,
    response_id: Option<String>,
    task_id: Option<String>,
    #[serde(default)]
    query: String,
    #[serde(default)]
    retrieved_context: String,
    #[serde(default)]
    assistant_response: String,
    before: LearningStateArgs,
    after: LearningStateArgs,
    outcome: LearningOutcomeArgs,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LearningStateArgs {
    topic_profile: [f32; NUM_EMBEDDERS],
    cross_correlations: Vec<f32>,
    retrieval_rank: Option<u32>,
    embedder_scores: Option<[f32; NUM_EMBEDDERS]>,
    #[serde(default)]
    contradiction_pressure: f32,
    #[serde(default)]
    integration_confidence: f32,
    #[serde(default)]
    recurrence_count: u32,
    #[serde(default)]
    stability_score: f32,
    domain: Option<String>,
    #[serde(default)]
    successful_transfer_count: u32,
}

impl From<LearningStateArgs> for LearningStateSnapshot {
    fn from(value: LearningStateArgs) -> Self {
        Self {
            topic_profile: value.topic_profile,
            cross_correlations: value.cross_correlations,
            retrieval_rank: value.retrieval_rank,
            embedder_scores: value.embedder_scores.unwrap_or([0.0; NUM_EMBEDDERS]),
            contradiction_pressure: value.contradiction_pressure,
            integration_confidence: value.integration_confidence,
            recurrence_count: value.recurrence_count,
            stability_score: value.stability_score,
            domain: value.domain,
            successful_transfer_count: value.successful_transfer_count,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LearningOutcomeArgs {
    label: String,
    #[serde(default)]
    utility_delta: f32,
    #[serde(default)]
    correction_required: bool,
    #[serde(default)]
    reuse_observed: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExportLearnerTrainingDatasetArgs {
    dataset_id: Option<Uuid>,
    #[serde(default = "default_training_task")]
    task: String,
    #[serde(default = "default_max_training_rows")]
    max_rows: usize,
    #[serde(default)]
    clear_existing: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListLearnerTrainingDatasetsArgs {
    #[serde(default = "default_list_limit")]
    limit: usize,
    #[serde(default)]
    offset: usize,
    #[serde(default = "default_true")]
    include_shape: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetLearnerTrainingDatasetArgs {
    dataset_id: Uuid,
    #[serde(default)]
    include_matrix: bool,
    #[serde(default = "default_preview_rows")]
    preview_rows: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EstimateLearningOutcomeArgs {
    before: LearningStateArgs,
    candidate_after: LearningStateArgs,
    #[serde(default = "default_max_neighbors")]
    max_neighbors: usize,
    #[serde(default = "default_outcome_max_scan")]
    max_scan: usize,
    #[serde(default)]
    min_similarity: f32,
    task_id: Option<String>,
    domain: Option<String>,
    #[serde(default = "default_true")]
    include_neighbors: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ComputeLearningSignalsArgs {
    before: LearningStateArgs,
    after: LearningStateArgs,
    outcome: LearningOutcomeArgs,
    signal_ids: Option<Vec<String>>,
    #[serde(default = "default_true")]
    include_features: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EmbedLearningEventSignalsArgs {
    event_id: Uuid,
    signal_ids: Option<Vec<String>>,
    #[serde(default = "default_true")]
    include_persisted: bool,
}

impl TryFrom<LearningOutcomeArgs> for LearningOutcome {
    type Error = String;

    fn try_from(value: LearningOutcomeArgs) -> Result<Self, Self::Error> {
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
    pub(crate) async fn call_record_learning_event(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let parsed: RecordLearningEventArgs = match serde_json::from_value(args) {
            Ok(v) => v,
            Err(e) => return self.tool_error(id, &format!("Invalid learning event args: {e}")),
        };

        let outcome = match LearningOutcome::try_from(parsed.outcome) {
            Ok(v) => v,
            Err(e) => return self.tool_error(id, &e),
        };

        let before: LearningStateSnapshot = parsed.before.into();
        let after: LearningStateSnapshot = parsed.after.into();
        let event_id = parsed.event_id.unwrap_or_else(Uuid::new_v4);

        let Some(rocksdb_store) = self
            .teleological_store
            .as_any()
            .downcast_ref::<RocksDbTeleologicalStore>()
        else {
            return self.tool_error(
                id,
                "record_learning_event requires RocksDbTeleologicalStore.",
            );
        };

        for memory_id in &parsed.memory_ids {
            match self.teleological_store.retrieve(*memory_id).await {
                Ok(Some(_)) => {}
                Ok(None) => {
                    let message = format!(
                        "record_learning_event memoryIds contains missing fingerprint: {memory_id}"
                    );
                    return self.tool_error(id, &message);
                }
                Err(e) => {
                    return self.tool_error(
                        id,
                        &format!(
                            "record_learning_event failed to read fingerprint {memory_id}: {e}"
                        ),
                    )
                }
            }
            match self.teleological_store.get_content(*memory_id).await {
                Ok(Some(content)) if !content.is_empty() => {}
                Ok(Some(_)) => {
                    let message = format!(
                        "record_learning_event memoryIds contains empty content row: {memory_id}"
                    );
                    return self.tool_error(id, &message);
                }
                Ok(None) => {
                    let message = format!(
                        "record_learning_event memoryIds contains missing content row: {memory_id}"
                    );
                    return self.tool_error(id, &message);
                }
                Err(e) => {
                    return self.tool_error(
                        id,
                        &format!("record_learning_event failed to read content {memory_id}: {e}"),
                    )
                }
            }
        }

        let event = match LearningEvent::new(
            event_id,
            parsed.memory_ids,
            parsed.session_id,
            parsed.response_id,
            parsed.task_id,
            parsed.query,
            parsed.retrieved_context,
            parsed.assistant_response,
            before,
            after,
            outcome,
        ) {
            Ok(v) => v,
            Err(e) => {
                return self.tool_error(id, &format!("Learning event validation failed: {e}"))
            }
        };

        if let Err(e) = rocksdb_store.store_learning_event(&event).await {
            error!(event_id = %event.event_id, error = %e, "store_learning_event failed");
            return self.tool_error(id, &format!("store_learning_event failed: {e}"));
        }

        self.tool_result(
            id,
            json!({
                "status": "stored",
                "event_id": event.event_id.to_string(),
                "source_of_truth": {
                    "backend": "rocksdb",
                    "column_family": "learning_events",
                    "format": "version_byte + bincode"
                },
                "shape": learning_event_shape(&event),
                "features": {
                    "delta_e_scalar": event.features.delta_e_scalar,
                    "surprise_score": event.features.surprise_score,
                    "coherence_delta": event.features.coherence_delta,
                    "consolidation_readiness": event.features.consolidation_readiness,
                    "transfer_score": event.features.transfer_score,
                    "multi_utl_score": event.features.multi_utl_score
                }
            }),
        )
    }

    pub(crate) async fn call_list_learning_events(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let limit = args
            .get("limit")
            .and_then(|v| v.as_u64())
            .unwrap_or(50)
            .min(1000) as usize;
        let offset = args.get("offset").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        let include_features = args
            .get("includeFeatures")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let Some(rocksdb_store) = self
            .teleological_store
            .as_any()
            .downcast_ref::<RocksDbTeleologicalStore>()
        else {
            return self.tool_error(
                id,
                "list_learning_events requires RocksDbTeleologicalStore.",
            );
        };

        let ids = match rocksdb_store.list_learning_event_ids().await {
            Ok(v) => v,
            Err(e) => return self.tool_error(id, &format!("list_learning_event_ids failed: {e}")),
        };
        let total = ids.len();
        let page: Vec<Uuid> = ids.into_iter().skip(offset).take(limit).collect();

        let mut items = Vec::with_capacity(page.len());
        for event_id in &page {
            if include_features {
                match rocksdb_store.get_learning_event(*event_id).await {
                    Ok(Some(event)) => items.push(json!({
                        "event_id": event_id.to_string(),
                        "shape": learning_event_shape(&event),
                        "features": {
                            "delta_e_scalar": event.features.delta_e_scalar,
                            "surprise_score": event.features.surprise_score,
                            "coherence_delta": event.features.coherence_delta,
                            "consolidation_readiness": event.features.consolidation_readiness,
                            "transfer_score": event.features.transfer_score,
                            "multi_utl_score": event.features.multi_utl_score
                        }
                    })),
                    Ok(None) => {
                        items.push(json!({"event_id": event_id.to_string(), "missing": true}))
                    }
                    Err(e) => {
                        return self.tool_error(
                            id,
                            &format!("get_learning_event failed for {event_id}: {e}"),
                        );
                    }
                }
            } else {
                items.push(json!({"event_id": event_id.to_string()}));
            }
        }

        self.tool_result(
            id,
            json!({
                "total": total,
                "offset": offset,
                "limit": limit,
                "returned": items.len(),
                "events": items
            }),
        )
    }

    pub(crate) async fn call_get_learning_event(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let event_id_raw = match args.get("eventId").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => return self.tool_error(id, "Missing required 'eventId' parameter"),
        };
        let event_id = match Uuid::parse_str(event_id_raw) {
            Ok(v) => v,
            Err(_) => return self.tool_error(id, "eventId must be a valid UUID"),
        };
        let include_text = args
            .get("includeText")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let include_signals = args
            .get("includeSignals")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let Some(rocksdb_store) = self
            .teleological_store
            .as_any()
            .downcast_ref::<RocksDbTeleologicalStore>()
        else {
            return self.tool_error(id, "get_learning_event requires RocksDbTeleologicalStore.");
        };

        match rocksdb_store.get_learning_event(event_id).await {
            Ok(Some(event)) => self.tool_result(
                id,
                render_learning_event(&event, include_text, include_signals),
            ),
            Ok(None) => self.tool_result(id, json!({"event_id": event_id_raw, "found": false})),
            Err(e) => self.tool_error(id, &format!("get_learning_event failed: {e}")),
        }
    }

    pub(crate) async fn call_count_learning_events(
        &self,
        id: Option<JsonRpcId>,
    ) -> JsonRpcResponse {
        let Some(rocksdb_store) = self
            .teleological_store
            .as_any()
            .downcast_ref::<RocksDbTeleologicalStore>()
        else {
            return self.tool_error(
                id,
                "count_learning_events requires RocksDbTeleologicalStore.",
            );
        };
        match rocksdb_store.count_learning_events().await {
            Ok(n) => self.tool_result(id, json!({"count": n})),
            Err(e) => self.tool_error(id, &format!("count_learning_events failed: {e}")),
        }
    }

    pub(crate) async fn call_list_learning_signal_embedders(
        &self,
        id: Option<JsonRpcId>,
    ) -> JsonRpcResponse {
        self.tool_result(
            id,
            json!({
                "source_of_truth": {
                    "crate": "context_graph_core::learning",
                    "trait": "LearningSignalEmbedder",
                    "implementation": "DeterministicLearningSignalEmbedder",
                },
                "count": all_learning_signal_ids().len(),
                "embedders": all_learning_signal_ids()
                    .iter()
                    .map(|signal_id| render_signal_embedder_metadata(*signal_id))
                    .collect::<Vec<_>>(),
            }),
        )
    }

    pub(crate) async fn call_compute_learning_signals(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let parsed: ComputeLearningSignalsArgs = match serde_json::from_value(args) {
            Ok(v) => v,
            Err(e) => {
                return self.tool_error(id, &format!("Invalid compute_learning_signals args: {e}"))
            }
        };
        let signal_ids = match parse_learning_signal_ids(parsed.signal_ids.as_deref()) {
            Ok(v) => v,
            Err(e) => return self.tool_error(id, &e),
        };
        let outcome = match LearningOutcome::try_from(parsed.outcome) {
            Ok(v) => v,
            Err(e) => return self.tool_error(id, &e),
        };
        let before: LearningStateSnapshot = parsed.before.into();
        let after: LearningStateSnapshot = parsed.after.into();
        let event = match LearningEvent::new(
            Uuid::nil(),
            Vec::new(),
            None,
            None,
            Some("inline-learning-signal-compute".into()),
            String::new(),
            String::new(),
            String::new(),
            before,
            after,
            outcome,
        ) {
            Ok(v) => v,
            Err(e) => {
                return self.tool_error(
                    id,
                    &format!("compute_learning_signals validation failed: {e}"),
                )
            }
        };
        let signals = match embed_selected_learning_signals(&event, &signal_ids).await {
            Ok(v) => v,
            Err(e) => return self.tool_error(id, &e),
        };
        let mut result = json!({
            "source_of_truth": {
                "input": "request.before + request.after + request.outcome",
                "crate": "context_graph_core::learning",
                "implementation": "DeterministicLearningSignalEmbedder",
                "storage_mutated": false,
            },
            "state_mutated": false,
            "shape": learning_event_shape(&event),
            "selected_signal_ids": signal_ids.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
            "signals": signals.iter().map(render_learning_signal).collect::<Vec<_>>(),
        });
        if parsed.include_features {
            result["features"] = render_learning_features(&event);
        }
        self.tool_result(id, result)
    }

    pub(crate) async fn call_embed_learning_event_signals(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let parsed: EmbedLearningEventSignalsArgs = match serde_json::from_value(args) {
            Ok(v) => v,
            Err(e) => {
                return self.tool_error(
                    id,
                    &format!("Invalid embed_learning_event_signals args: {e}"),
                )
            }
        };
        let signal_ids = match parse_learning_signal_ids(parsed.signal_ids.as_deref()) {
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
                "embed_learning_event_signals requires RocksDbTeleologicalStore.",
            );
        };

        let before_count = match rocksdb_store.count_learning_events().await {
            Ok(v) => v,
            Err(e) => return self.tool_error(id, &format!("count_learning_events failed: {e}")),
        };
        let event = match rocksdb_store.get_learning_event(parsed.event_id).await {
            Ok(Some(v)) => v,
            Ok(None) => {
                return self.tool_error(
                    id,
                    &format!(
                        "LearningEvent {} not found in CF_LEARNING_EVENTS",
                        parsed.event_id
                    ),
                )
            }
            Err(e) => return self.tool_error(id, &format!("get_learning_event failed: {e}")),
        };
        let signals = match embed_selected_learning_signals(&event, &signal_ids).await {
            Ok(v) => v,
            Err(e) => return self.tool_error(id, &e),
        };
        let after_count = match rocksdb_store.count_learning_events().await {
            Ok(v) => v,
            Err(e) => {
                return self
                    .tool_error(id, &format!("post-embed count_learning_events failed: {e}"))
            }
        };
        let comparisons = signals
            .iter()
            .map(|signal| {
                let persisted = event
                    .signals
                    .iter()
                    .find(|candidate| candidate.signal_id == signal.signal_id);
                let mut value = json!({
                    "signal_id": signal.signal_id.as_str(),
                    "persisted_present": persisted.is_some(),
                    "persisted_match": persisted == Some(signal),
                });
                if parsed.include_persisted {
                    value["persisted"] = json!(persisted.map(render_learning_signal));
                }
                value["fresh"] = render_learning_signal(signal);
                value
            })
            .collect::<Vec<_>>();

        self.tool_result(
            id,
            json!({
                "source_of_truth": {
                    "backend": "rocksdb",
                    "column_family": "learning_events",
                    "format": "version_byte + bincode",
                    "event_id": parsed.event_id.to_string(),
                },
                "before_count": before_count,
                "after_count": after_count,
                "state_mutated": before_count != after_count,
                "event_id": parsed.event_id.to_string(),
                "shape": learning_event_shape(&event),
                "selected_signal_ids": signal_ids.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
                "persisted_all_match": comparisons
                    .iter()
                    .all(|entry| entry["persisted_match"].as_bool().unwrap_or(false)),
                "signals": comparisons,
            }),
        )
    }

    pub(crate) async fn call_estimate_learning_outcome(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let parsed: EstimateLearningOutcomeArgs = match serde_json::from_value(args) {
            Ok(v) => v,
            Err(e) => {
                return self.tool_error(id, &format!("Invalid estimate_learning_outcome args: {e}"))
            }
        };
        if parsed.max_neighbors == 0 || parsed.max_neighbors > 100 {
            return self.tool_error(id, "maxNeighbors must be in [1, 100]");
        }
        if parsed.max_scan == 0 || parsed.max_scan > 200_000 {
            return self.tool_error(id, "maxScan must be in [1, 200000]");
        }
        if !parsed.min_similarity.is_finite() || !(-1.0..=1.0).contains(&parsed.min_similarity) {
            return self.tool_error(id, "minSimilarity must be finite and in [-1, 1]");
        }

        let before: LearningStateSnapshot = parsed.before.into();
        let candidate_after: LearningStateSnapshot = parsed.candidate_after.into();
        if let Err(e) = before.validate("before") {
            return self.tool_error(id, &format!("before validation failed: {e}"));
        }
        if let Err(e) = candidate_after.validate("candidateAfter") {
            return self.tool_error(id, &format!("candidateAfter validation failed: {e}"));
        }
        let query_case = outcome_case_vector(&before, &candidate_after);

        let Some(rocksdb_store) = self
            .teleological_store
            .as_any()
            .downcast_ref::<RocksDbTeleologicalStore>()
        else {
            return self.tool_error(
                id,
                "estimate_learning_outcome requires RocksDbTeleologicalStore.",
            );
        };

        let before_count = match rocksdb_store.count_learning_events().await {
            Ok(v) => v,
            Err(e) => return self.tool_error(id, &format!("count_learning_events failed: {e}")),
        };
        if before_count == 0 {
            return self.tool_error(
                id,
                "CF_LEARNING_EVENTS is empty; cannot estimate an outcome without persisted cases",
            );
        }
        let ids = match rocksdb_store.list_learning_event_ids().await {
            Ok(v) => v,
            Err(e) => return self.tool_error(id, &format!("list_learning_event_ids failed: {e}")),
        };

        let mut scored = Vec::new();
        let mut scanned = 0usize;
        for event_id in ids.into_iter().take(parsed.max_scan) {
            let event = match rocksdb_store.get_learning_event(event_id).await {
                Ok(Some(v)) => v,
                Ok(None) => {
                    return self.tool_error(
                        id,
                        &format!(
                        "list_learning_event_ids returned {event_id}, but point read was missing"
                    ),
                    )
                }
                Err(e) => {
                    return self.tool_error(
                        id,
                        &format!("get_learning_event failed for {event_id}: {e}"),
                    )
                }
            };
            scanned += 1;
            if let Some(task_id) = parsed.task_id.as_ref() {
                if event.task_id.as_deref() != Some(task_id.as_str()) {
                    continue;
                }
            }
            if let Some(domain) = parsed.domain.as_ref() {
                let event_domain_match = event.before.domain.as_deref() == Some(domain.as_str())
                    || event.after.domain.as_deref() == Some(domain.as_str());
                if !event_domain_match {
                    continue;
                }
            }
            let event_case = outcome_case_vector(&event.before, &event.after);
            let similarity = cosine_similarity(&query_case, &event_case);
            if !similarity.is_finite() {
                return self.tool_error(
                    id,
                    &format!("case similarity for event {event_id} was non-finite"),
                );
            }
            if similarity >= parsed.min_similarity {
                scored.push((similarity, event));
            }
        }
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        let neighbors = scored
            .into_iter()
            .filter(|(similarity, _)| *similarity > 0.0)
            .take(parsed.max_neighbors)
            .collect::<Vec<_>>();
        if neighbors.is_empty() {
            return self.tool_error(
                id,
                "No positive-similarity LearningEvent neighbors matched the requested filters",
            );
        }

        let mut weighted_sum = 0.0f32;
        let mut weight_sum = 0.0f32;
        let mut neighbor_json = Vec::new();
        for (similarity, event) in &neighbors {
            let weight = similarity * similarity;
            weighted_sum += weight * event.outcome.utility_delta;
            weight_sum += weight;
            if parsed.include_neighbors {
                neighbor_json.push(json!({
                    "event_id": event.event_id.to_string(),
                    "similarity": similarity,
                    "weight": weight,
                    "utility_delta": event.outcome.utility_delta,
                    "outcome_label": outcome_label_str(event.outcome.label),
                    "task_id": event.task_id,
                    "before_domain": event.before.domain,
                    "after_domain": event.after.domain,
                    "delta_e_scalar": event.features.delta_e_scalar,
                    "coherence_delta": event.features.coherence_delta,
                    "embedder_disagreement": event.features.embedder_disagreement
                }));
            }
        }
        if weight_sum <= 0.0 {
            return self.tool_error(id, "Matched neighbors had zero total similarity weight");
        }
        let predicted_utility_delta = (weighted_sum / weight_sum).clamp(-1.0, 1.0);
        let mean_similarity = neighbors
            .iter()
            .map(|(similarity, _)| *similarity)
            .sum::<f32>()
            / neighbors.len() as f32;
        let confidence =
            (mean_similarity * (neighbors.len() as f32 / 3.0).min(1.0)).clamp(0.0, 1.0);
        let after_count = match rocksdb_store.count_learning_events().await {
            Ok(v) => v,
            Err(e) => {
                return self.tool_error(
                    id,
                    &format!("post-estimate count_learning_events failed: {e}"),
                )
            }
        };

        self.tool_result(
            id,
            json!({
                "source_of_truth": {
                    "backend": "rocksdb",
                    "column_family": "learning_events",
                    "format": "version_byte + bincode"
                },
                "before_count": before_count,
                "after_count": after_count,
                "state_mutated": before_count != after_count,
                "scanned_events": scanned,
                "matched_neighbors": neighbors.len(),
                "feature_dimensions": query_case.len(),
                "predicted_utility_delta": predicted_utility_delta,
                "predicted_label": predicted_outcome_label(predicted_utility_delta),
                "confidence": confidence,
                "mean_neighbor_similarity": mean_similarity,
                "neighbors": neighbor_json,
            }),
        )
    }

    pub(crate) async fn call_export_learner_training_dataset(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let parsed: ExportLearnerTrainingDatasetArgs = match serde_json::from_value(args) {
            Ok(v) => v,
            Err(e) => {
                return self.tool_error(
                    id,
                    &format!("Invalid export_learner_training_dataset args: {e}"),
                )
            }
        };
        let task = match LearnerTrainingTask::parse(&parsed.task) {
            Ok(v) => v,
            Err(e) => return self.tool_error(id, &format!("{e}")),
        };
        if parsed.max_rows == 0 {
            return self.tool_error(
                id,
                "export_learner_training_dataset requires maxRows > 0; empty exports are not persisted",
            );
        }
        let Some(rocksdb_store) = self
            .teleological_store
            .as_any()
            .downcast_ref::<RocksDbTeleologicalStore>()
        else {
            return self.tool_error(
                id,
                "export_learner_training_dataset requires RocksDbTeleologicalStore.",
            );
        };

        let build = match build_training_matrix(rocksdb_store, task, parsed.max_rows).await {
            Ok(v) => v,
            Err(e) => return self.tool_error(id, &e),
        };
        if build.rows.is_empty() {
            return self.tool_error(
                id,
                &format!(
                    "export_learner_training_dataset found zero eligible rows for task={} with source_counts={:?}; no dataset was written",
                    task.as_str(),
                    build.source_counts
                ),
            );
        }

        let cleared = if parsed.clear_existing {
            match rocksdb_store.clear_all_learner_training_datasets().await {
                Ok(v) => v,
                Err(e) => {
                    return self.tool_error(
                        id,
                        &format!("clear_all_learner_training_datasets failed: {e}"),
                    )
                }
            }
        } else {
            0
        };
        let dataset = match LearnerTrainingDataset::new(
            parsed.dataset_id.unwrap_or_else(Uuid::new_v4),
            task,
            build.feature_schema,
            build.label_schema,
            build.rows,
            build.row_major,
            build.source_counts,
            BTreeMap::from([("max_rows".into(), parsed.max_rows.to_string())]),
        ) {
            Ok(v) => v,
            Err(e) => {
                return self.tool_error(
                    id,
                    &format!("LearnerTrainingDataset validation failed: {e}"),
                )
            }
        };
        if let Err(e) = rocksdb_store.store_learner_training_dataset(&dataset).await {
            return self.tool_error(id, &format!("store_learner_training_dataset failed: {e}"));
        }
        self.tool_result(
            id,
            json!({
                "status": "stored",
                "dataset_id": dataset.dataset_id.to_string(),
                "source_of_truth": {
                    "backend": "rocksdb",
                    "column_family": "learner_training_datasets",
                    "format": "version_byte + bincode"
                },
                "cleared_before_export": cleared,
                "task": dataset.task.as_str(),
                "rows": dataset.rows_len,
                "cols": dataset.cols_len,
                "row_major_values": dataset.row_major.len(),
                "row_major_sha256": dataset.row_major_sha256,
                "provenance_manifest_sha256": dataset.provenance_manifest_sha256,
                "source_counts": dataset.source_counts,
            }),
        )
    }

    pub(crate) async fn call_list_learner_training_datasets(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let parsed: ListLearnerTrainingDatasetsArgs = match serde_json::from_value(args) {
            Ok(v) => v,
            Err(e) => {
                return self.tool_error(
                    id,
                    &format!("Invalid list_learner_training_datasets args: {e}"),
                )
            }
        };
        let Some(rocksdb_store) = self
            .teleological_store
            .as_any()
            .downcast_ref::<RocksDbTeleologicalStore>()
        else {
            return self.tool_error(
                id,
                "list_learner_training_datasets requires RocksDbTeleologicalStore.",
            );
        };
        let ids = match rocksdb_store.list_learner_training_dataset_ids().await {
            Ok(v) => v,
            Err(e) => {
                return self.tool_error(
                    id,
                    &format!("list_learner_training_dataset_ids failed: {e}"),
                )
            }
        };
        let total = ids.len();
        let page = ids
            .into_iter()
            .skip(parsed.offset)
            .take(parsed.limit.min(1000))
            .collect::<Vec<_>>();
        let mut datasets = Vec::with_capacity(page.len());
        for dataset_id in &page {
            if parsed.include_shape {
                match rocksdb_store
                    .get_learner_training_dataset(*dataset_id)
                    .await
                {
                    Ok(Some(dataset)) => datasets.push(render_training_dataset_summary(&dataset)),
                    Ok(None) => datasets
                        .push(json!({"dataset_id": dataset_id.to_string(), "missing": true})),
                    Err(e) => {
                        return self.tool_error(
                            id,
                            &format!("get_learner_training_dataset failed for {dataset_id}: {e}"),
                        )
                    }
                }
            } else {
                datasets.push(json!({"dataset_id": dataset_id.to_string()}));
            }
        }
        self.tool_result(
            id,
            json!({
                "source_of_truth": {
                    "backend": "rocksdb",
                    "column_family": "learner_training_datasets",
                    "format": "version_byte + bincode"
                },
                "total": total,
                "offset": parsed.offset,
                "limit": parsed.limit.min(1000),
                "returned": datasets.len(),
                "datasets": datasets,
            }),
        )
    }

    pub(crate) async fn call_get_learner_training_dataset(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let parsed: GetLearnerTrainingDatasetArgs = match serde_json::from_value(args) {
            Ok(v) => v,
            Err(e) => {
                return self.tool_error(
                    id,
                    &format!("Invalid get_learner_training_dataset args: {e}"),
                )
            }
        };
        let Some(rocksdb_store) = self
            .teleological_store
            .as_any()
            .downcast_ref::<RocksDbTeleologicalStore>()
        else {
            return self.tool_error(
                id,
                "get_learner_training_dataset requires RocksDbTeleologicalStore.",
            );
        };
        match rocksdb_store
            .get_learner_training_dataset(parsed.dataset_id)
            .await
        {
            Ok(Some(dataset)) => self.tool_result(
                id,
                render_training_dataset(&dataset, parsed.include_matrix, parsed.preview_rows),
            ),
            Ok(None) => self.tool_result(
                id,
                json!({"dataset_id": parsed.dataset_id.to_string(), "found": false}),
            ),
            Err(e) => self.tool_error(id, &format!("get_learner_training_dataset failed: {e}")),
        }
    }

    pub(crate) async fn call_count_learner_training_datasets(
        &self,
        id: Option<JsonRpcId>,
    ) -> JsonRpcResponse {
        let Some(rocksdb_store) = self
            .teleological_store
            .as_any()
            .downcast_ref::<RocksDbTeleologicalStore>()
        else {
            return self.tool_error(
                id,
                "count_learner_training_datasets requires RocksDbTeleologicalStore.",
            );
        };
        match rocksdb_store.count_learner_training_datasets().await {
            Ok(n) => self.tool_result(
                id,
                json!({
                    "source_of_truth": {
                        "backend": "rocksdb",
                        "column_family": "learner_training_datasets",
                        "format": "version_byte + bincode"
                    },
                    "count": n
                }),
            ),
            Err(e) => self.tool_error(id, &format!("count_learner_training_datasets failed: {e}")),
        }
    }
}

fn learning_event_shape(event: &LearningEvent) -> serde_json::Value {
    json!({
        "before_topic_profile_len": event.before.topic_profile.len(),
        "after_topic_profile_len": event.after.topic_profile.len(),
        "before_cross_correlations_len": event.before.cross_correlations.len(),
        "after_cross_correlations_len": event.after.cross_correlations.len(),
        "delta_e_vector_len": event.features.delta_e_vector.len(),
        "attribution_len": event.features.attribution.len(),
        "signals": event.signals.iter().map(|s| json!({
            "signal_id": s.signal_id.as_str(),
            "vector_len": s.vector.len(),
            "scalar": s.scalar,
        })).collect::<Vec<_>>(),
        "memory_ids": event.memory_ids.len()
    })
}

fn all_learning_signal_ids() -> [LearningSignalId; 5] {
    [
        LearningSignalId::DeltaE,
        LearningSignalId::Surprise,
        LearningSignalId::Coherence,
        LearningSignalId::Consolidation,
        LearningSignalId::Transfer,
    ]
}

fn parse_learning_signal_id(raw: &str) -> Result<LearningSignalId, String> {
    match raw {
        "delta_e" => Ok(LearningSignalId::DeltaE),
        "surprise" => Ok(LearningSignalId::Surprise),
        "coherence" => Ok(LearningSignalId::Coherence),
        "consolidation" => Ok(LearningSignalId::Consolidation),
        "transfer" => Ok(LearningSignalId::Transfer),
        other => Err(format!(
            "signalIds entries must be delta_e, surprise, coherence, consolidation, or transfer; got {other}"
        )),
    }
}

fn parse_learning_signal_ids(raw: Option<&[String]>) -> Result<Vec<LearningSignalId>, String> {
    let selected = match raw {
        Some([]) => return Err("signalIds must not be empty when provided".into()),
        Some(values) => values
            .iter()
            .map(|value| parse_learning_signal_id(value.as_str()))
            .collect::<Result<Vec<_>, _>>()?,
        None => all_learning_signal_ids().to_vec(),
    };
    let mut seen = std::collections::BTreeSet::new();
    for signal_id in &selected {
        if !seen.insert(signal_id.as_str()) {
            return Err(format!("duplicate signalIds entry: {}", signal_id.as_str()));
        }
    }
    Ok(selected)
}

async fn embed_selected_learning_signals(
    event: &LearningEvent,
    signal_ids: &[LearningSignalId],
) -> Result<Vec<LearningSignal>, String> {
    let mut out = Vec::with_capacity(signal_ids.len());
    for signal_id in signal_ids {
        let embedder = DeterministicLearningSignalEmbedder::new(*signal_id);
        let signal = embedder.embed_event(event).await.map_err(|e| {
            format!(
                "learning signal embedder {} failed: {e}",
                signal_id.as_str()
            )
        })?;
        signal.validate().map_err(|e| {
            format!(
                "learning signal {} validation failed: {e}",
                signal_id.as_str()
            )
        })?;
        out.push(signal);
    }
    Ok(out)
}

fn render_signal_embedder_metadata(signal_id: LearningSignalId) -> serde_json::Value {
    json!({
        "signal_id": signal_id.as_str(),
        "dimension": signal_id.dimension(),
        "implementation": "DeterministicLearningSignalEmbedder",
        "input": "LearningEvent",
        "storage_scope": "event_level_utl_not_e1_e14",
    })
}

fn render_learning_signal(signal: &LearningSignal) -> serde_json::Value {
    json!({
        "signal_id": signal.signal_id.as_str(),
        "dimension": signal.signal_id.dimension(),
        "vector": signal.vector,
        "scalar": signal.scalar,
        "label": signal.label,
        "attribution": signal.attribution,
    })
}

fn render_learning_features(event: &LearningEvent) -> serde_json::Value {
    json!({
        "delta_e_vector": event.features.delta_e_vector,
        "delta_e_scalar": event.features.delta_e_scalar,
        "retrieval_rank_shift": event.features.retrieval_rank_shift,
        "embedder_disagreement": event.features.embedder_disagreement,
        "surprise_score": event.features.surprise_score,
        "productive_surprise": event.features.productive_surprise,
        "coherence_delta": event.features.coherence_delta,
        "contradiction_delta": event.features.contradiction_delta,
        "consolidation_readiness": event.features.consolidation_readiness,
        "transfer_score": event.features.transfer_score,
        "multi_utl_score": event.features.multi_utl_score,
        "attribution": event.features.attribution,
    })
}

fn render_learning_event(
    event: &LearningEvent,
    include_text: bool,
    include_signals: bool,
) -> serde_json::Value {
    let mut obj = json!({
        "event_id": event.event_id.to_string(),
        "found": true,
        "created_at": event.created_at,
        "memory_ids": event.memory_ids.iter().map(ToString::to_string).collect::<Vec<_>>(),
        "session_id": event.session_id,
        "response_id": event.response_id,
        "task_id": event.task_id,
        "before": render_state(&event.before),
        "after": render_state(&event.after),
        "outcome": {
            "label": format!("{:?}", event.outcome.label),
            "utility_delta": event.outcome.utility_delta,
            "correction_required": event.outcome.correction_required,
            "reuse_observed": event.outcome.reuse_observed,
        },
        "features": render_learning_features(event),
        "shape": learning_event_shape(event),
    });

    if include_text {
        obj["query"] = json!(event.query);
        obj["retrieved_context"] = json!(event.retrieved_context);
        obj["assistant_response"] = json!(event.assistant_response);
    }

    if include_signals {
        obj["signals"] = json!(event
            .signals
            .iter()
            .map(render_learning_signal)
            .collect::<Vec<_>>());
    }

    obj
}

fn render_state(state: &LearningStateSnapshot) -> serde_json::Value {
    json!({
        "topic_profile": state.topic_profile,
        "cross_correlations_len": state.cross_correlations.len(),
        "retrieval_rank": state.retrieval_rank,
        "embedder_scores": state.embedder_scores,
        "contradiction_pressure": state.contradiction_pressure,
        "integration_confidence": state.integration_confidence,
        "recurrence_count": state.recurrence_count,
        "stability_score": state.stability_score,
        "domain": state.domain,
        "successful_transfer_count": state.successful_transfer_count,
    })
}

struct MatrixBuild {
    feature_schema: Vec<String>,
    label_schema: Vec<String>,
    rows: Vec<LearnerTrainingRow>,
    row_major: Vec<f32>,
    source_counts: BTreeMap<String, u64>,
}

async fn build_training_matrix(
    store: &RocksDbTeleologicalStore,
    task: LearnerTrainingTask,
    max_rows: usize,
) -> Result<MatrixBuild, String> {
    match task {
        LearnerTrainingTask::RewardModel
        | LearnerTrainingTask::Reranker
        | LearnerTrainingTask::EmbedderContrastive => {
            build_learning_event_matrix(store, task, max_rows).await
        }
        LearnerTrainingTask::DiagnosticClassifier => build_diagnostic_matrix(store, max_rows).await,
        LearnerTrainingTask::Scheduler => build_scheduler_matrix(store, max_rows).await,
        LearnerTrainingTask::PersonalPhysiology => {
            build_personal_physiology_matrix(store, max_rows).await
        }
    }
}

async fn build_learning_event_matrix(
    store: &RocksDbTeleologicalStore,
    task: LearnerTrainingTask,
    max_rows: usize,
) -> Result<MatrixBuild, String> {
    let feature_schema = learning_event_feature_schema();
    let label_schema = learning_event_label_schema();
    let ids = store
        .list_learning_event_ids()
        .await
        .map_err(|e| format!("list_learning_event_ids failed: {e}"))?;
    let mut rows = Vec::new();
    let mut row_major = Vec::new();
    for event_id in ids.into_iter().take(max_rows) {
        let event = store
            .get_learning_event(event_id)
            .await
            .map_err(|e| format!("get_learning_event failed for {event_id}: {e}"))?
            .ok_or_else(|| format!("learning event id {event_id} was listed but not readable"))?;
        let features = learning_event_feature_vector(&event)
            .map_err(|e| format!("learning_event_feature_vector failed for {event_id}: {e}"))?;
        assert_feature_len(&features, &feature_schema, "learning_event")?;
        row_major.extend_from_slice(&features);
        let label_scalar = match task {
            LearnerTrainingTask::EmbedderContrastive => Some(event.features.transfer_score),
            _ => Some(event.outcome.utility_delta),
        };
        rows.push(LearnerTrainingRow {
            row_id: event.event_id,
            source_cf: "learning_events".into(),
            source_key: event.event_id.to_string(),
            event_id: Some(event.event_id),
            learner_id: None,
            session_ts: None,
            label_scalar,
            label_class: Some(outcome_label_str(event.outcome.label).into()),
            split_key: event
                .session_id
                .clone()
                .or(event.task_id.clone())
                .unwrap_or_else(|| "unscoped-learning-event".into()),
            provenance_sha256: sha256_json(&event)
                .map_err(|e| format!("hash learning event {event_id} failed: {e}"))?,
        });
    }
    Ok(MatrixBuild {
        feature_schema,
        label_schema,
        source_counts: BTreeMap::from([("learning_events".into(), rows.len() as u64)]),
        rows,
        row_major,
    })
}

async fn build_diagnostic_matrix(
    store: &RocksDbTeleologicalStore,
    max_rows: usize,
) -> Result<MatrixBuild, String> {
    let feature_schema = diagnostic_feature_schema();
    let label_schema = vec!["diagnostic_state".into(), "l".into()];
    let keys = store
        .list_learner_delta_log_keys()
        .await
        .map_err(|e| format!("list_learner_delta_log_keys failed: {e}"))?;
    let mut rows = Vec::new();
    let mut row_major = Vec::new();
    for (learner_id, session_ts) in keys.into_iter().take(max_rows) {
        let log = store
            .get_learner_delta_log(learner_id, session_ts)
            .await
            .map_err(|e| {
                format!("get_learner_delta_log failed for {learner_id}/{session_ts}: {e}")
            })?
            .ok_or_else(|| {
                format!("delta log key {learner_id}/{session_ts} was listed but not readable")
            })?;
        let state = store
            .get_learner_state_vector(learner_id, session_ts)
            .await
            .map_err(|e| {
                format!("get_learner_state_vector failed for {learner_id}/{session_ts}: {e}")
            })?
            .ok_or_else(|| {
                format!(
                    "diagnostic export requires learner_state_history for {learner_id}/{session_ts}"
                )
            })?;
        let features = diagnostic_feature_vector(&log, &state);
        assert_feature_len(&features, &feature_schema, "diagnostic")?;
        row_major.extend_from_slice(&features);
        rows.push(LearnerTrainingRow {
            row_id: Uuid::new_v4(),
            source_cf: "learner_delta_log".into(),
            source_key: format!("{learner_id}:{session_ts}"),
            event_id: None,
            learner_id: Some(learner_id),
            session_ts: Some(session_ts),
            label_scalar: Some(log.computation.l),
            label_class: Some(log.computation.diagnostic_state.as_str().into()),
            split_key: learner_id.to_string(),
            provenance_sha256: sha256_json(&log)
                .map_err(|e| format!("hash delta log {learner_id}/{session_ts} failed: {e}"))?,
        });
    }
    Ok(MatrixBuild {
        feature_schema,
        label_schema,
        source_counts: BTreeMap::from([("learner_delta_log".into(), rows.len() as u64)]),
        rows,
        row_major,
    })
}

async fn build_scheduler_matrix(
    store: &RocksDbTeleologicalStore,
    max_rows: usize,
) -> Result<MatrixBuild, String> {
    let feature_schema = scheduler_feature_schema();
    let label_schema = vec!["retrieval_score".into(), "correct".into()];
    let keys = store
        .list_learner_retrieval_log_keys()
        .await
        .map_err(|e| format!("list_learner_retrieval_log_keys failed: {e}"))?;
    let mut rows = Vec::new();
    let mut row_major = Vec::new();
    for (learner_id, trace_id, ts) in keys.into_iter().take(max_rows) {
        let log = store
            .get_learner_retrieval_log(learner_id, trace_id, ts)
            .await
            .map_err(|e| {
                format!("get_learner_retrieval_log failed for {learner_id}/{trace_id}/{ts}: {e}")
            })?
            .ok_or_else(|| {
                format!(
                    "retrieval log key {learner_id}/{trace_id}/{ts} was listed but not readable"
                )
            })?;
        let features = scheduler_feature_vector(&log);
        assert_feature_len(&features, &feature_schema, "scheduler")?;
        row_major.extend_from_slice(&features);
        rows.push(LearnerTrainingRow {
            row_id: Uuid::new_v4(),
            source_cf: "learner_retrieval_log".into(),
            source_key: format!("{learner_id}:{trace_id}:{ts}"),
            event_id: None,
            learner_id: Some(learner_id),
            session_ts: Some(log.state_at_retrieval.session_ts),
            label_scalar: Some(log.score),
            label_class: Some(if log.correct { "correct" } else { "incorrect" }.into()),
            split_key: learner_id.to_string(),
            provenance_sha256: sha256_json(&log).map_err(|e| {
                format!("hash retrieval log {learner_id}/{trace_id}/{ts} failed: {e}")
            })?,
        });
    }
    Ok(MatrixBuild {
        feature_schema,
        label_schema,
        source_counts: BTreeMap::from([("learner_retrieval_log".into(), rows.len() as u64)]),
        rows,
        row_major,
    })
}

async fn build_personal_physiology_matrix(
    store: &RocksDbTeleologicalStore,
    max_rows: usize,
) -> Result<MatrixBuild, String> {
    let feature_schema = personal_physiology_feature_schema();
    let label_schema = vec!["personal_calibration_row".into()];
    let keys = store
        .list_learner_fingerprint_keys()
        .await
        .map_err(|e| format!("list_learner_fingerprint_keys failed: {e}"))?;
    let mut rows = Vec::new();
    let mut row_major = Vec::new();
    for (learner_id, session_ts) in keys.into_iter().take(max_rows) {
        let fingerprint = store
            .get_learner_fingerprint(learner_id, session_ts)
            .await
            .map_err(|e| {
                format!("get_learner_fingerprint failed for {learner_id}/{session_ts}: {e}")
            })?
            .ok_or_else(|| {
                format!("fingerprint key {learner_id}/{session_ts} was listed but not readable")
            })?;
        let features = personal_physiology_feature_vector(&fingerprint);
        assert_feature_len(&features, &feature_schema, "personal_physiology")?;
        row_major.extend_from_slice(&features);
        rows.push(LearnerTrainingRow {
            row_id: Uuid::new_v4(),
            source_cf: "fingerprints_learner".into(),
            source_key: format!("{learner_id}:{session_ts}"),
            event_id: None,
            learner_id: Some(learner_id),
            session_ts: Some(session_ts),
            label_scalar: None,
            label_class: Some("personal_calibration_row".into()),
            split_key: learner_id.to_string(),
            provenance_sha256: sha256_json(&fingerprint).map_err(|e| {
                format!("hash learner fingerprint {learner_id}/{session_ts} failed: {e}")
            })?,
        });
    }
    Ok(MatrixBuild {
        feature_schema,
        label_schema,
        source_counts: BTreeMap::from([("fingerprints_learner".into(), rows.len() as u64)]),
        rows,
        row_major,
    })
}

fn diagnostic_feature_schema() -> Vec<String> {
    vec![
        "plasticity_window",
        "hrv_coherence",
        "valence",
        "arousal",
        "stress_floor",
        "k_sleep",
        "delta_s",
        "delta_c",
        "delta_e",
        "l",
        "outcome_stability",
        "coefficient_of_variation",
        "gradient_effectiveness",
        "valence_arousal",
        "k_state",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

fn diagnostic_feature_vector(
    log: &context_graph_core::learner::LearnerDeltaLog,
    state: &context_graph_core::learner::LearnerStateVector,
) -> Vec<f32> {
    vec![
        state.components.plasticity_window,
        state.components.hrv_coherence,
        state.components.valence,
        state.components.arousal,
        state.components.stress_floor,
        state.components.k_sleep,
        log.computation.delta_s.delta_s,
        log.computation.delta_c.delta_c,
        log.computation.delta_e.delta_e,
        log.computation.l,
        log.computation.delta_c.outcome_stability,
        log.computation.delta_c.coefficient_of_variation,
        log.computation.delta_c.gradient_effectiveness,
        log.computation.delta_e.valence_arousal,
        log.computation.delta_e.k_state,
    ]
}

fn scheduler_feature_schema() -> Vec<String> {
    vec![
        "plasticity_window",
        "hrv_coherence",
        "valence",
        "arousal",
        "stress_floor",
        "k_sleep",
        "retrieval_score",
        "retrieval_correct_flag",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

fn scheduler_feature_vector(log: &context_graph_core::learner::LearnerRetrievalLog) -> Vec<f32> {
    vec![
        log.state_at_retrieval.components.plasticity_window,
        log.state_at_retrieval.components.hrv_coherence,
        log.state_at_retrieval.components.valence,
        log.state_at_retrieval.components.arousal,
        log.state_at_retrieval.components.stress_floor,
        log.state_at_retrieval.components.k_sleep,
        log.score,
        if log.correct { 1.0 } else { 0.0 },
    ]
}

fn personal_physiology_feature_schema() -> Vec<String> {
    let mut schema = vec![
        "plasticity_window".into(),
        "hrv_coherence".into(),
        "valence".into(),
        "arousal".into(),
        "stress_floor".into(),
        "k_sleep".into(),
    ];
    for modality in learner_embedder_modalities() {
        schema.push(format!("{}_present", modality.as_str()));
        schema.push(format!("{}_scalar", modality.as_str()));
        schema.push(format!("{}_vector_norm", modality.as_str()));
    }
    schema
}

fn personal_physiology_feature_vector(
    fingerprint: &context_graph_core::learner::LearnerFingerprint,
) -> Vec<f32> {
    let mut features = vec![
        fingerprint.state_vector.components.plasticity_window,
        fingerprint.state_vector.components.hrv_coherence,
        fingerprint.state_vector.components.valence,
        fingerprint.state_vector.components.arousal,
        fingerprint.state_vector.components.stress_floor,
        fingerprint.state_vector.components.k_sleep,
    ];
    for modality in learner_embedder_modalities() {
        if let Some(embedding) = fingerprint
            .modality_embeddings
            .iter()
            .find(|embedding| embedding.modality == modality)
        {
            features.push(1.0);
            features.push(embedding.scalar.unwrap_or(0.0));
            features.push(vector_norm(&embedding.vector));
        } else {
            features.extend_from_slice(&[0.0, 0.0, 0.0]);
        }
    }
    features
}

fn learner_embedder_modalities() -> [LearnerModality; 7] {
    [
        LearnerModality::AffectSpeech,
        LearnerModality::AffectFace,
        LearnerModality::AffectText,
        LearnerModality::Ppg,
        LearnerModality::Eda,
        LearnerModality::Eeg,
        LearnerModality::EegArtifactRobust,
    ]
}

fn assert_feature_len(features: &[f32], schema: &[String], row_kind: &str) -> Result<(), String> {
    if features.len() != schema.len() {
        return Err(format!(
            "{row_kind} feature length {} does not match schema length {}",
            features.len(),
            schema.len()
        ));
    }
    if features.iter().any(|v| !v.is_finite()) {
        return Err(format!(
            "{row_kind} feature vector contains a non-finite value"
        ));
    }
    Ok(())
}

fn vector_norm(values: &[f32]) -> f32 {
    values
        .iter()
        .map(|value| (*value as f64) * (*value as f64))
        .sum::<f64>()
        .sqrt() as f32
}

fn outcome_label_str(label: LearningOutcomeLabel) -> &'static str {
    match label {
        LearningOutcomeLabel::Useful => "useful",
        LearningOutcomeLabel::Neutral => "neutral",
        LearningOutcomeLabel::Harmful => "harmful",
        LearningOutcomeLabel::NoLearning => "no_learning",
    }
}

fn predicted_outcome_label(utility_delta: f32) -> &'static str {
    if utility_delta >= 0.15 {
        "useful"
    } else if utility_delta <= -0.15 {
        "harmful"
    } else if utility_delta.abs() <= 0.05 {
        "no_learning"
    } else {
        "neutral"
    }
}

fn outcome_case_vector(before: &LearningStateSnapshot, after: &LearningStateSnapshot) -> Vec<f32> {
    let mut out = Vec::with_capacity(NUM_EMBEDDERS * 5 + before.cross_correlations.len() * 2 + 10);
    out.extend_from_slice(&before.topic_profile);
    out.extend_from_slice(&after.topic_profile);
    for i in 0..NUM_EMBEDDERS {
        out.push(after.topic_profile[i] - before.topic_profile[i]);
    }
    out.extend_from_slice(&before.cross_correlations);
    for (before_value, after_value) in before
        .cross_correlations
        .iter()
        .zip(after.cross_correlations.iter())
    {
        out.push(after_value - before_value);
    }
    out.extend_from_slice(&before.embedder_scores);
    out.extend_from_slice(&after.embedder_scores);
    out.push(rank_norm(before.retrieval_rank));
    out.push(rank_norm(after.retrieval_rank));
    out.push(after.contradiction_pressure - before.contradiction_pressure);
    out.push(after.integration_confidence - before.integration_confidence);
    out.push(after.stability_score - before.stability_score);
    out.push(before.recurrence_count as f32 / 1000.0);
    out.push(after.recurrence_count as f32 / 1000.0);
    out.push(before.successful_transfer_count as f32 / 1000.0);
    out.push(after.successful_transfer_count as f32 / 1000.0);
    out
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0f32;
    let mut na = 0.0f32;
    let mut nb = 0.0f32;
    for i in 0..a.len() {
        dot += a[i] * b[i];
        na += a[i] * a[i];
        nb += b[i] * b[i];
    }
    if na <= 0.0 || nb <= 0.0 {
        return 0.0;
    }
    (dot / (na.sqrt() * nb.sqrt())).clamp(-1.0, 1.0)
}

fn rank_norm(rank: Option<u32>) -> f32 {
    rank.map(|rank| 1.0 / (1.0 + rank as f32)).unwrap_or(0.0)
}

fn render_training_dataset_summary(dataset: &LearnerTrainingDataset) -> serde_json::Value {
    json!({
        "dataset_id": dataset.dataset_id.to_string(),
        "task": dataset.task.as_str(),
        "created_at": dataset.created_at,
        "rows": dataset.rows_len,
        "cols": dataset.cols_len,
        "row_major_values": dataset.row_major.len(),
        "row_major_sha256": dataset.row_major_sha256,
        "provenance_manifest_sha256": dataset.provenance_manifest_sha256,
        "source_counts": dataset.source_counts,
    })
}

fn render_training_dataset(
    dataset: &LearnerTrainingDataset,
    include_matrix: bool,
    preview_rows: usize,
) -> serde_json::Value {
    let preview_rows = preview_rows.min(100).min(dataset.rows.len());
    let cols = dataset.cols_len as usize;
    let row_preview = (0..preview_rows)
        .map(|row_idx| {
            let start = row_idx * cols;
            let end = start + cols;
            json!({
                "row": dataset.rows[row_idx],
                "features": dataset.row_major[start..end].to_vec(),
            })
        })
        .collect::<Vec<_>>();
    let mut out = json!({
        "found": true,
        "source_of_truth": {
            "backend": "rocksdb",
            "column_family": "learner_training_datasets",
            "format": "version_byte + bincode"
        },
        "dataset": render_training_dataset_summary(dataset),
        "feature_schema": dataset.feature_schema,
        "label_schema": dataset.label_schema,
        "filters": dataset.filters,
        "preview_rows": row_preview,
    });
    if include_matrix {
        out["row_major"] = json!(dataset.row_major);
    }
    out
}

fn default_training_task() -> String {
    "reward_model".into()
}

fn default_max_training_rows() -> usize {
    10_000
}

fn default_list_limit() -> usize {
    50
}

fn default_preview_rows() -> usize {
    3
}

fn default_max_neighbors() -> usize {
    5
}

fn default_outcome_max_scan() -> usize {
    10_000
}

fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use context_graph_core::learning::{LearningOutcome, LearningOutcomeLabel};
    use context_graph_core::training::NUM_CROSS_CORRELATIONS;
    use tempfile::TempDir;

    fn state(value: f32, rank: u32) -> LearningStateSnapshot {
        LearningStateSnapshot {
            topic_profile: [value; NUM_EMBEDDERS],
            cross_correlations: vec![value * 0.5; NUM_CROSS_CORRELATIONS],
            retrieval_rank: Some(rank),
            embedder_scores: [value; NUM_EMBEDDERS],
            contradiction_pressure: 0.1,
            integration_confidence: 0.7,
            recurrence_count: 2,
            stability_score: 0.8,
            domain: Some("synthetic-fsv-domain".into()),
            successful_transfer_count: 1,
        }
    }

    fn event(event_id: Uuid, before: f32, after: f32, utility_delta: f32) -> LearningEvent {
        LearningEvent::new(
            event_id,
            vec![Uuid::from_u128(0x11111111_2222_4333_8444_555555555555)],
            Some("fsv-session".into()),
            Some(format!("response-{event_id}")),
            Some("reward-model-fsv".into()),
            "What does controlled FSV prove?".into(),
            "FSV proves persisted state by separate readback.".into(),
            "It proves the expected row exists in RocksDB.".into(),
            state(before, 5),
            state(after, 2),
            LearningOutcome {
                label: LearningOutcomeLabel::Useful,
                utility_delta,
                correction_required: false,
                reuse_observed: true,
            },
        )
        .unwrap()
    }

    #[tokio::test]
    async fn learner_training_dataset_fsv_happy_path_and_edges() {
        let tempdir = TempDir::new().unwrap();
        let store = RocksDbTeleologicalStore::open(tempdir.path()).unwrap();

        println!("SOURCE OF TRUTH: RocksDB CF_LEARNER_TRAINING_DATASETS");
        println!(
            "HAPPY BEFORE count={}",
            store.count_learner_training_datasets().await.unwrap()
        );

        let event_id = Uuid::from_u128(0xaaaaaaaa_bbbb_4ccc_8ddd_eeeeeeeeeeee);
        let first_event = event(event_id, 0.2, 0.4, 0.75);
        store.store_learning_event(&first_event).await.unwrap();

        let build = build_training_matrix(&store, LearnerTrainingTask::RewardModel, 10)
            .await
            .unwrap();
        let expected_cols = learning_event_feature_schema().len();
        assert_eq!(build.rows.len(), 1);
        assert_eq!(build.row_major.len(), expected_cols);
        assert_eq!(build.row_major[0], 0.2);

        let dataset_id = Uuid::from_u128(0xbbbbbbbb_bbbb_4bbb_8bbb_bbbbbbbbbbbb);
        let dataset = LearnerTrainingDataset::new(
            dataset_id,
            LearnerTrainingTask::RewardModel,
            build.feature_schema,
            build.label_schema,
            build.rows,
            build.row_major,
            build.source_counts,
            BTreeMap::from([("case".into(), "happy".into())]),
        )
        .unwrap();
        store
            .store_learner_training_dataset(&dataset)
            .await
            .unwrap();

        let readback = store
            .get_learner_training_dataset(dataset_id)
            .await
            .unwrap()
            .expect("dataset must be physically present in CF");
        println!(
            "HAPPY AFTER count={} rows={} cols={} first_feature={} label={} sha={}",
            store.count_learner_training_datasets().await.unwrap(),
            readback.rows_len,
            readback.cols_len,
            readback.row_major[0],
            readback.rows[0].label_scalar.unwrap(),
            readback.row_major_sha256
        );
        assert_eq!(readback.rows_len, 1);
        assert_eq!(readback.cols_len as usize, expected_cols);
        assert_eq!(readback.rows[0].event_id, Some(event_id));
        assert_eq!(readback.rows[0].label_scalar, Some(0.75));

        println!(
            "EDGE EMPTY BEFORE count={}",
            store.count_learner_training_datasets().await.unwrap()
        );
        let empty_temp = TempDir::new().unwrap();
        let empty_store = RocksDbTeleologicalStore::open(empty_temp.path()).unwrap();
        let empty_build = build_training_matrix(&empty_store, LearnerTrainingTask::RewardModel, 10)
            .await
            .unwrap();
        let empty_dataset_id = Uuid::from_u128(0xcccccccc_cccc_4ccc_8ccc_cccccccccccc);
        let empty_dataset = LearnerTrainingDataset::new(
            empty_dataset_id,
            LearnerTrainingTask::RewardModel,
            empty_build.feature_schema,
            empty_build.label_schema,
            empty_build.rows,
            empty_build.row_major,
            empty_build.source_counts,
            BTreeMap::from([("case".into(), "empty".into())]),
        )
        .unwrap();
        empty_store
            .store_learner_training_dataset(&empty_dataset)
            .await
            .unwrap();
        let empty_readback = empty_store
            .get_learner_training_dataset(empty_dataset_id)
            .await
            .unwrap()
            .unwrap();
        println!(
            "EDGE EMPTY AFTER count={} rows={} row_major_len={}",
            empty_store.count_learner_training_datasets().await.unwrap(),
            empty_readback.rows_len,
            empty_readback.row_major.len()
        );
        assert_eq!(empty_readback.rows_len, 0);
        assert_eq!(empty_readback.row_major.len(), 0);

        println!(
            "EDGE LIMIT BEFORE learning_events={} datasets={}",
            store.count_learning_events().await.unwrap(),
            store.count_learner_training_datasets().await.unwrap()
        );
        store
            .store_learning_event(&event(
                Uuid::from_u128(0xdddddddd_dddd_4ddd_8ddd_dddddddddddd),
                0.3,
                0.5,
                0.25,
            ))
            .await
            .unwrap();
        let limited_build = build_training_matrix(&store, LearnerTrainingTask::RewardModel, 1)
            .await
            .unwrap();
        let limited_dataset_id = Uuid::from_u128(0xeeeeeeee_eeee_4eee_8eee_eeeeeeeeeeee);
        let limited_dataset = LearnerTrainingDataset::new(
            limited_dataset_id,
            LearnerTrainingTask::RewardModel,
            limited_build.feature_schema,
            limited_build.label_schema,
            limited_build.rows,
            limited_build.row_major,
            limited_build.source_counts,
            BTreeMap::from([("case".into(), "limit".into())]),
        )
        .unwrap();
        store
            .store_learner_training_dataset(&limited_dataset)
            .await
            .unwrap();
        let limited_readback = store
            .get_learner_training_dataset(limited_dataset_id)
            .await
            .unwrap()
            .unwrap();
        println!(
            "EDGE LIMIT AFTER learning_events={} dataset_rows={} expected_rows=1",
            store.count_learning_events().await.unwrap(),
            limited_readback.rows_len
        );
        assert_eq!(limited_readback.rows_len, 1);

        let invalid_before = store.count_learner_training_datasets().await.unwrap();
        println!("EDGE INVALID BEFORE datasets={invalid_before}");
        let invalid = LearnerTrainingDataset::new(
            Uuid::from_u128(0xffffeeee_dddd_4ccc_8bbb_aaaaaaaaaaaa),
            LearnerTrainingTask::RewardModel,
            vec!["x".into(), "y".into()],
            vec!["label".into()],
            vec![LearnerTrainingRow {
                row_id: Uuid::new_v4(),
                source_cf: "learning_events".into(),
                source_key: event_id.to_string(),
                event_id: Some(event_id),
                learner_id: None,
                session_ts: None,
                label_scalar: Some(0.0),
                label_class: Some("useful".into()),
                split_key: "invalid".into(),
                provenance_sha256: readback.rows[0].provenance_sha256.clone(),
            }],
            vec![1.0],
            BTreeMap::new(),
            BTreeMap::from([("case".into(), "invalid".into())]),
        );
        assert!(invalid.is_err());
        let invalid_after = store.count_learner_training_datasets().await.unwrap();
        println!(
            "EDGE INVALID AFTER datasets={invalid_after} error={:?}",
            invalid.err()
        );
        assert_eq!(invalid_after, invalid_before);
    }
}
