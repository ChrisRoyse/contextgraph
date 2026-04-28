//! Learning-as-UTL event tool handlers.

use std::collections::BTreeMap;

use context_graph_core::learner::{sha256_json, LearnerModality};
use context_graph_core::learner_training::{
    learning_event_feature_schema, learning_event_feature_vector, learning_event_label_schema,
    LearnerTrainingDataset, LearnerTrainingRow, LearnerTrainingTask,
};
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

        let build = match build_training_matrix(rocksdb_store, task, parsed.max_rows).await {
            Ok(v) => v,
            Err(e) => return self.tool_error(id, &e),
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
