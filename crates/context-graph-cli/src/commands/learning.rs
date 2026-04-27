//! CLI inspection tools for `CF_LEARNING_EVENTS`.

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use context_graph_core::learning::{
    LearningEvent, LearningOutcome, LearningOutcomeLabel, LearningStateSnapshot,
};
use context_graph_core::training::NUM_CROSS_CORRELATIONS;
use context_graph_core::types::fingerprint::NUM_EMBEDDERS;
use context_graph_storage::teleological::{RocksDbTeleologicalStore, TeleologicalStoreConfig};
use serde_json::json;
use uuid::Uuid;

#[derive(Subcommand, Debug)]
pub enum LearningCommands {
    /// Count rows in CF_LEARNING_EVENTS.
    Count(LearningStorageArgs),
    /// List rows in CF_LEARNING_EVENTS.
    List(LearningListArgs),
    /// Fetch one learning event by UUID.
    Get(LearningGetArgs),
    /// Store a deterministic synthetic learning event and read it back.
    RecordSynthetic(LearningRecordSyntheticArgs),
}

#[derive(Args, Debug)]
pub struct LearningStorageArgs {
    /// Path to the RocksDB data directory.
    #[arg(long, env = "CONTEXT_GRAPH_STORAGE_PATH")]
    pub storage: PathBuf,
}

#[derive(Args, Debug)]
pub struct LearningListArgs {
    /// Path to the RocksDB data directory.
    #[arg(long, env = "CONTEXT_GRAPH_STORAGE_PATH")]
    pub storage: PathBuf,

    /// Max rows to return.
    #[arg(long, default_value_t = 50)]
    pub limit: usize,

    /// Rows to skip.
    #[arg(long, default_value_t = 0)]
    pub offset: usize,
}

#[derive(Args, Debug)]
pub struct LearningGetArgs {
    /// Path to the RocksDB data directory.
    #[arg(long, env = "CONTEXT_GRAPH_STORAGE_PATH")]
    pub storage: PathBuf,

    /// Learning event UUID.
    #[arg(long)]
    pub event_id: Uuid,

    /// Include query/context/response text.
    #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
    pub include_text: bool,
}

#[derive(Args, Debug)]
pub struct LearningRecordSyntheticArgs {
    /// Path to the RocksDB data directory. Created by RocksDB when missing.
    #[arg(long, env = "CONTEXT_GRAPH_STORAGE_PATH")]
    pub storage: PathBuf,

    /// Optional deterministic event UUID.
    #[arg(long)]
    pub event_id: Option<Uuid>,

    /// Delete existing learning events before writing the synthetic row.
    #[arg(long)]
    pub clear_existing: bool,
}

pub async fn handle_learning_command(action: LearningCommands) -> i32 {
    match run(action).await {
        Ok(value) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string())
            );
            0
        }
        Err(e) => {
            eprintln!("learning command FAILED: {:#}", e);
            1
        }
    }
}

async fn run(action: LearningCommands) -> Result<serde_json::Value> {
    match action {
        LearningCommands::Count(args) => {
            let store = open_store(&args.storage)?;
            let count = store.count_learning_events().await?;
            Ok(json!({
                "source_of_truth": source_of_truth(),
                "count": count,
            }))
        }
        LearningCommands::List(args) => {
            let store = open_store(&args.storage)?;
            let ids = store.list_learning_event_ids().await?;
            let total = ids.len();
            let page: Vec<_> = ids.into_iter().skip(args.offset).take(args.limit).collect();
            let mut events = Vec::with_capacity(page.len());
            for id in page {
                let event = store
                    .get_learning_event(id)
                    .await?
                    .with_context(|| format!("listed event {id} was not readable"))?;
                events.push(json!({
                    "event_id": id.to_string(),
                    "shape": event_shape(&event),
                    "features": feature_summary(&event),
                }));
            }
            Ok(json!({
                "source_of_truth": source_of_truth(),
                "total": total,
                "offset": args.offset,
                "limit": args.limit,
                "returned": events.len(),
                "events": events,
            }))
        }
        LearningCommands::Get(args) => {
            let store = open_store(&args.storage)?;
            let event = store.get_learning_event(args.event_id).await?;
            Ok(match event {
                Some(event) => render_event(&event, args.include_text),
                None => json!({
                    "source_of_truth": source_of_truth(),
                    "event_id": args.event_id.to_string(),
                    "found": false,
                }),
            })
        }
        LearningCommands::RecordSynthetic(args) => {
            let store = open_store(&args.storage)?;
            let cleared = if args.clear_existing {
                store.clear_all_learning_events().await?
            } else {
                0
            };
            let event = synthetic_event(args.event_id.unwrap_or_else(Uuid::new_v4))?;
            let before_count = store.count_learning_events().await?;
            store.store_learning_event(&event).await?;
            let after_count = store.count_learning_events().await?;
            let readback = store
                .get_learning_event(event.event_id)
                .await?
                .with_context(|| "synthetic event missing after write")?;
            Ok(json!({
                "source_of_truth": source_of_truth(),
                "cleared_before_write": cleared,
                "before_count": before_count,
                "after_count": after_count,
                "stored_event_id": event.event_id.to_string(),
                "readback": render_event(&readback, true),
            }))
        }
    }
}

fn open_store(storage: &PathBuf) -> Result<RocksDbTeleologicalStore> {
    RocksDbTeleologicalStore::open_with_config(storage, TeleologicalStoreConfig::default())
        .with_context(|| format!("opening RocksDbTeleologicalStore at {}", storage.display()))
}

fn synthetic_event(event_id: Uuid) -> Result<LearningEvent> {
    let mut before_profile = [0.0f32; NUM_EMBEDDERS];
    let mut after_profile = [0.0f32; NUM_EMBEDDERS];
    let mut before_scores = [0.0f32; NUM_EMBEDDERS];
    let mut after_scores = [0.0f32; NUM_EMBEDDERS];
    for idx in 0..NUM_EMBEDDERS {
        before_profile[idx] = (idx as f32 + 1.0) / 100.0;
        after_profile[idx] = before_profile[idx] + 0.1;
        before_scores[idx] = 0.2;
        after_scores[idx] = 0.2 + (idx as f32 / (NUM_EMBEDDERS - 1) as f32) * 0.7;
    }

    let before = LearningStateSnapshot {
        topic_profile: before_profile,
        cross_correlations: vec![0.05; NUM_CROSS_CORRELATIONS],
        retrieval_rank: Some(10),
        embedder_scores: before_scores,
        contradiction_pressure: 0.2,
        integration_confidence: 0.4,
        recurrence_count: 1,
        stability_score: 0.4,
        domain: Some("docs".into()),
        successful_transfer_count: 0,
    };
    let after = LearningStateSnapshot {
        topic_profile: after_profile,
        cross_correlations: vec![0.15; NUM_CROSS_CORRELATIONS],
        retrieval_rank: Some(2),
        embedder_scores: after_scores,
        contradiction_pressure: 0.1,
        integration_confidence: 0.8,
        recurrence_count: 8,
        stability_score: 0.9,
        domain: Some("code".into()),
        successful_transfer_count: 3,
    };
    let outcome = LearningOutcome {
        label: LearningOutcomeLabel::Useful,
        utility_delta: 0.4,
        correction_required: false,
        reuse_observed: true,
    };
    Ok(LearningEvent::new(
        event_id,
        vec![Uuid::new_v4()],
        Some("synthetic-learning-fsv".into()),
        Some("synthetic-response".into()),
        Some("synthetic-task".into()),
        "How does the parser state move after adding a deterministic feature?".into(),
        "Synthetic context with known before and after profiles.".into(),
        "The feature improves integration and transfers from docs to code.".into(),
        before,
        after,
        outcome,
    )?)
}

fn source_of_truth() -> serde_json::Value {
    json!({
        "backend": "rocksdb",
        "column_family": "learning_events",
        "format": "version_byte + bincode",
    })
}

fn event_shape(event: &LearningEvent) -> serde_json::Value {
    json!({
        "before_topic_profile_len": event.before.topic_profile.len(),
        "after_topic_profile_len": event.after.topic_profile.len(),
        "before_cross_correlations_len": event.before.cross_correlations.len(),
        "after_cross_correlations_len": event.after.cross_correlations.len(),
        "delta_e_vector_len": event.features.delta_e_vector.len(),
        "signals": event.signals.iter().map(|s| json!({
            "signal_id": s.signal_id.as_str(),
            "vector_len": s.vector.len(),
            "scalar": s.scalar,
        })).collect::<Vec<_>>(),
    })
}

fn feature_summary(event: &LearningEvent) -> serde_json::Value {
    json!({
        "delta_e_scalar": event.features.delta_e_scalar,
        "retrieval_rank_shift": event.features.retrieval_rank_shift,
        "surprise_score": event.features.surprise_score,
        "coherence_delta": event.features.coherence_delta,
        "consolidation_readiness": event.features.consolidation_readiness,
        "transfer_score": event.features.transfer_score,
        "multi_utl_score": event.features.multi_utl_score,
    })
}

fn render_event(event: &LearningEvent, include_text: bool) -> serde_json::Value {
    let mut value = json!({
        "source_of_truth": source_of_truth(),
        "event_id": event.event_id.to_string(),
        "found": true,
        "created_at": event.created_at,
        "session_id": event.session_id,
        "response_id": event.response_id,
        "task_id": event.task_id,
        "memory_ids": event.memory_ids.iter().map(ToString::to_string).collect::<Vec<_>>(),
        "before": {
            "topic_profile": event.before.topic_profile,
            "cross_correlations_len": event.before.cross_correlations.len(),
            "retrieval_rank": event.before.retrieval_rank,
            "embedder_scores": event.before.embedder_scores,
            "contradiction_pressure": event.before.contradiction_pressure,
            "integration_confidence": event.before.integration_confidence,
            "recurrence_count": event.before.recurrence_count,
            "stability_score": event.before.stability_score,
            "domain": event.before.domain,
            "successful_transfer_count": event.before.successful_transfer_count,
        },
        "after": {
            "topic_profile": event.after.topic_profile,
            "cross_correlations_len": event.after.cross_correlations.len(),
            "retrieval_rank": event.after.retrieval_rank,
            "embedder_scores": event.after.embedder_scores,
            "contradiction_pressure": event.after.contradiction_pressure,
            "integration_confidence": event.after.integration_confidence,
            "recurrence_count": event.after.recurrence_count,
            "stability_score": event.after.stability_score,
            "domain": event.after.domain,
            "successful_transfer_count": event.after.successful_transfer_count,
        },
        "outcome": {
            "label": format!("{:?}", event.outcome.label),
            "utility_delta": event.outcome.utility_delta,
            "correction_required": event.outcome.correction_required,
            "reuse_observed": event.outcome.reuse_observed,
        },
        "shape": event_shape(event),
        "features": feature_summary(event),
        "signals": event.signals.iter().map(|s| json!({
            "signal_id": s.signal_id.as_str(),
            "vector": s.vector,
            "scalar": s.scalar,
            "label": s.label,
        })).collect::<Vec<_>>(),
    });

    if include_text {
        value["query"] = json!(event.query);
        value["retrieved_context"] = json!(event.retrieved_context);
        value["assistant_response"] = json!(event.assistant_response);
    }
    value
}
