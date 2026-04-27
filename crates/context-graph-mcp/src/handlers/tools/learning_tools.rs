//! Learning-as-UTL event tool handlers.

use context_graph_core::learning::{
    LearningEvent, LearningOutcome, LearningOutcomeLabel, LearningStateSnapshot,
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
        "features": {
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
        },
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
            .map(|s| json!({
                "signal_id": s.signal_id.as_str(),
                "vector": s.vector,
                "scalar": s.scalar,
                "label": s.label,
                "attribution": s.attribution,
            }))
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
