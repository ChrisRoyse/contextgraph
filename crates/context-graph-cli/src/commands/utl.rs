//! CLI inspection tools for persisted UTL learner-state records.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use context_graph_core::learner::{
    compute_delta_c, compute_delta_e, compute_delta_s_from_text, compute_utl_l, sha256_bytes,
    sha256_json, update_m_trace, ComputationEnvelope, LearnerAuditEntry, LearnerDeltaLog,
    LearnerFingerprint, LearnerModality, LearnerProfile, LearnerStateComponents,
    ObservationEnvelope,
};
use context_graph_embeddings::{
    embed_calibration_sample, learner_embedder_specs, state_vector_from_outputs,
    synthetic_calibration_fixture, LearnerEmbedderSlot, CALIBRATION_DATASET_MANIFEST,
    UTL_PLANNED_TOTAL_EMBEDDERS,
};
use context_graph_storage::teleological::{RocksDbTeleologicalStore, TeleologicalStoreConfig};
use serde_json::json;
use uuid::Uuid;

#[derive(Subcommand, Debug)]
pub enum UtlCommands {
    /// List the learner-state E14-E20 embedder registry and calibration datasets.
    ListEmbedders,
    /// Print the deterministic local calibration fixture summary.
    Fixture,
    /// Count rows in the learner-state source-of-truth column families.
    Count(UtlStorageArgs),
    /// Store a deterministic synthetic learner session and read it back.
    RecordSynthetic(UtlRecordSyntheticArgs),
    /// Fetch one persisted UTL delta log.
    GetDelta(UtlGetSessionArgs),
    /// Fetch one persisted M(t) trace.
    GetM(UtlGetTraceArgs),
}

#[derive(Args, Debug)]
pub struct UtlStorageArgs {
    /// Path to the RocksDB data directory.
    #[arg(long, env = "CONTEXT_GRAPH_STORAGE_PATH")]
    pub storage: PathBuf,
}

#[derive(Args, Debug)]
pub struct UtlRecordSyntheticArgs {
    /// Path to the RocksDB data directory. Created by RocksDB when missing.
    #[arg(long, env = "CONTEXT_GRAPH_STORAGE_PATH")]
    pub storage: PathBuf,

    /// Optional deterministic learner UUID.
    #[arg(long)]
    pub learner_id: Option<Uuid>,

    /// Optional deterministic trace UUID.
    #[arg(long)]
    pub trace_id: Option<Uuid>,

    /// Synthetic session timestamp.
    #[arg(long, default_value_t = 1_700_000_000)]
    pub session_ts: u64,

    /// Delete existing learner-state records before writing the synthetic rows.
    #[arg(long)]
    pub clear_existing: bool,
}

#[derive(Args, Debug)]
pub struct UtlGetSessionArgs {
    /// Path to the RocksDB data directory.
    #[arg(long, env = "CONTEXT_GRAPH_STORAGE_PATH")]
    pub storage: PathBuf,

    /// Learner UUID.
    #[arg(long)]
    pub learner_id: Uuid,

    /// Session timestamp.
    #[arg(long)]
    pub session_ts: u64,
}

#[derive(Args, Debug)]
pub struct UtlGetTraceArgs {
    /// Path to the RocksDB data directory.
    #[arg(long, env = "CONTEXT_GRAPH_STORAGE_PATH")]
    pub storage: PathBuf,

    /// Learner UUID.
    #[arg(long)]
    pub learner_id: Uuid,

    /// Trace UUID.
    #[arg(long)]
    pub trace_id: Uuid,
}

pub async fn handle_utl_command(action: UtlCommands) -> i32 {
    match run(action).await {
        Ok(value) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string())
            );
            0
        }
        Err(e) => {
            eprintln!("utl command FAILED: {:#}", e);
            1
        }
    }
}

async fn run(action: UtlCommands) -> Result<serde_json::Value> {
    match action {
        UtlCommands::ListEmbedders => Ok(json!({
            "source_of_truth": {
                "registry": "context_graph_embeddings::learner",
                "fixture_manifest": "crates/context-graph-embeddings/fixtures/utl_calibration"
            },
            "planned_total_embedders": UTL_PLANNED_TOTAL_EMBEDDERS,
            "learner_embedder_count": learner_embedder_specs().len(),
            "learner_embedders": learner_embedder_specs().iter().map(|spec| json!({
                "slot": spec.slot_number,
                "id": spec.slot.as_str(),
                "modality": spec.modality.as_str(),
                "model_name": spec.model_name,
                "model_path": spec.model_path,
                "output_dimension": spec.output_dimension,
                "scalar_heads": spec.scalar_heads,
            })).collect::<Vec<_>>(),
            "calibration_datasets": CALIBRATION_DATASET_MANIFEST.iter().map(|dataset| json!({
                "name": dataset.name,
                "modality": dataset.modality,
                "used_for": dataset.used_for,
                "access": dataset.access,
                "license": dataset.license,
            })).collect::<Vec<_>>(),
        })),
        UtlCommands::Fixture => {
            let fixture = synthetic_calibration_fixture();
            Ok(json!({
                "source_of_truth": {
                    "rust": "context_graph_embeddings::learner::synthetic_calibration_fixture",
                    "checked_in_manifest": "crates/context-graph-embeddings/fixtures/utl_calibration/phase0_synthetic_fixture.json",
                },
                "fixture_id": fixture.fixture_id,
                "sample_count": fixture.samples.len(),
                "samples": fixture.samples.iter().map(|sample| json!({
                    "sample_id": sample.sample_id,
                    "label": sample.label,
                    "input_slots": sample.inputs.keys().map(|slot| slot.as_str()).collect::<Vec<_>>(),
                    "expected_components": sample.expected_components,
                })).collect::<Vec<_>>()
            }))
        }
        UtlCommands::Count(args) => {
            let store = open_store(&args.storage)?;
            Ok(json!({
                "source_of_truth": source_of_truth(),
                "counts": counts(&store).await?,
            }))
        }
        UtlCommands::RecordSynthetic(args) => {
            let store = open_store(&args.storage)?;
            let cleared = if args.clear_existing {
                store.clear_all_learner_state_records().await?
            } else {
                0
            };
            let before_counts = counts(&store).await?;
            let learner_id = args
                .learner_id
                .unwrap_or_else(|| Uuid::from_u128(0xaaaaaaaa_aaaa_4aaa_8aaa_aaaaaaaaaaaa));
            let trace_id = args
                .trace_id
                .unwrap_or_else(|| Uuid::from_u128(0xbbbbbbbb_bbbb_4bbb_8bbb_bbbbbbbbbbbb));
            let synthetic = synthetic_records(learner_id, trace_id, args.session_ts)?;

            store.store_learner_profile(&synthetic.profile).await?;
            store
                .store_learner_fingerprint(&synthetic.fingerprint)
                .await?;
            store.store_learner_delta_log(&synthetic.delta_log).await?;
            store.store_learner_m_trace(&synthetic.m_trace).await?;
            store
                .store_learner_audit_entry(&synthetic.audit_entry)
                .await?;

            let after_counts = counts(&store).await?;
            let readback_profile = store
                .get_learner_profile(learner_id)
                .await?
                .with_context(|| "synthetic learner profile missing after write")?;
            let readback_fingerprint = store
                .get_learner_fingerprint(learner_id, args.session_ts)
                .await?
                .with_context(|| "synthetic learner fingerprint missing after write")?;
            let readback_delta = store
                .get_learner_delta_log(learner_id, args.session_ts)
                .await?
                .with_context(|| "synthetic delta log missing after write")?;
            let readback_m = store
                .get_learner_m_trace(learner_id, trace_id)
                .await?
                .with_context(|| "synthetic M trace missing after write")?;

            Ok(json!({
                "source_of_truth": source_of_truth(),
                "cleared_before_write": cleared,
                "before_counts": before_counts,
                "after_counts": after_counts,
                "learner_id": learner_id.to_string(),
                "trace_id": trace_id.to_string(),
                "session_ts": args.session_ts,
                "readback": {
                    "profile": render_profile(&readback_profile),
                    "fingerprint": render_fingerprint(&readback_fingerprint),
                    "delta_log": render_delta_log(&readback_delta),
                    "m_trace": render_m_trace(&readback_m),
                }
            }))
        }
        UtlCommands::GetDelta(args) => {
            let store = open_store(&args.storage)?;
            let delta = store
                .get_learner_delta_log(args.learner_id, args.session_ts)
                .await?;
            Ok(match delta {
                Some(delta) => json!({
                    "source_of_truth": source_of_truth(),
                    "found": true,
                    "delta_log": render_delta_log(&delta),
                }),
                None => json!({
                    "source_of_truth": source_of_truth(),
                    "found": false,
                    "learner_id": args.learner_id.to_string(),
                    "session_ts": args.session_ts,
                }),
            })
        }
        UtlCommands::GetM(args) => {
            let store = open_store(&args.storage)?;
            let trace = store
                .get_learner_m_trace(args.learner_id, args.trace_id)
                .await?;
            Ok(match trace {
                Some(trace) => json!({
                    "source_of_truth": source_of_truth(),
                    "found": true,
                    "m_trace": render_m_trace(&trace),
                }),
                None => json!({
                    "source_of_truth": source_of_truth(),
                    "found": false,
                    "learner_id": args.learner_id.to_string(),
                    "trace_id": args.trace_id.to_string(),
                }),
            })
        }
    }
}

fn open_store(storage: &PathBuf) -> Result<RocksDbTeleologicalStore> {
    RocksDbTeleologicalStore::open_with_config(storage, TeleologicalStoreConfig::default())
        .with_context(|| format!("opening RocksDbTeleologicalStore at {}", storage.display()))
}

async fn counts(store: &RocksDbTeleologicalStore) -> Result<serde_json::Value> {
    Ok(json!({
        "learner_profile": store.count_learner_profiles().await?,
        "fingerprints_learner": store.count_learner_fingerprints().await?,
        "learner_state_history": store.count_learner_state_history().await?,
        "learner_delta_log": store.count_learner_delta_logs().await?,
        "learner_m_per_trace": store.count_learner_m_traces().await?,
        "learner_audit": store.count_learner_audit_entries().await?,
    }))
}

struct SyntheticRecords {
    profile: LearnerProfile,
    fingerprint: LearnerFingerprint,
    delta_log: LearnerDeltaLog,
    m_trace: context_graph_core::learner::LearnerMTrace,
    audit_entry: LearnerAuditEntry,
}

fn synthetic_records(
    learner_id: Uuid,
    trace_id: Uuid,
    session_ts: u64,
) -> Result<SyntheticRecords> {
    let modalities = LearnerEmbedderSlot::all()
        .iter()
        .map(|slot| slot.modality())
        .chain(std::iter::once(LearnerModality::SelfReport))
        .collect::<BTreeSet<_>>();
    let profile = LearnerProfile::new(
        learner_id,
        "synthetic-all-e14-e20-learner".into(),
        "consented-local-first".into(),
        modalities,
        Some(session_ts),
    )?;

    let fixture = synthetic_calibration_fixture();
    let sample = fixture
        .samples
        .iter()
        .find(|sample| sample.sample_id == "regulated-baseline")
        .with_context(|| "regulated synthetic calibration sample missing")?;
    let outputs = embed_calibration_sample(sample)?;
    let state_vector = state_vector_from_outputs(
        learner_id,
        session_ts,
        &outputs,
        BTreeMap::from([
            ("environment".into(), "quiet-dev-workstation".into()),
            ("fixture_id".into(), fixture.fixture_id.into()),
            ("sample_id".into(), sample.sample_id.into()),
            ("task".into(), "synthetic-all-e14-e20-utl-fsv".into()),
        ]),
    )?;

    let mut observation_envelopes = Vec::with_capacity(outputs.len());
    let mut modality_embeddings = Vec::with_capacity(outputs.len());
    for output in &outputs {
        let observation_id = Uuid::from_u128(
            0xcccccccc_cccc_4ccc_8ccc_cccccccccc00 + u128::from(output.slot.slot_number()),
        );
        let raw_hash =
            sha256_bytes(format!("{}:{}", sample.sample_id, output.slot.as_str()).as_bytes());
        let envelope = ObservationEnvelope::new(
            observation_id,
            learner_id,
            session_ts,
            output.modality,
            "consented-local-first".into(),
            raw_hash,
            "synthetic-preprocess-v1".into(),
            output.embedder_version.clone(),
            "thresholds-default-pending-calibration-v1".into(),
            Vec::new(),
        )?;
        observation_envelopes.push(envelope);
        modality_embeddings.push(output.to_modality_embedding(observation_id));
    }

    let components: LearnerStateComponents = state_vector.components.clone();
    let fingerprint = LearnerFingerprint {
        learner_id,
        session_ts,
        observation_envelopes,
        modality_embeddings,
        state_vector,
    };

    let delta_s = compute_delta_s_from_text(
        "ephemeral means temporary",
        "ephemeral means lasting only a short time",
        Some("temporary short lived concept"),
        0.2,
        Some(0.7),
    )?;
    let delta_c = compute_delta_c(
        &[0.62, 0.74, 0.86],
        components.hrv_coherence,
        0.82,
        0.05,
        None,
    )?;
    let delta_e = compute_delta_e(&components)?;
    let computation = compute_utl_l(delta_s, delta_c, delta_e, 0, None)?;
    let output_hash = sha256_json(&computation)?;
    let provenance = ComputationEnvelope::new(
        Uuid::from_u128(0xdddddddd_dddd_4ddd_8ddd_dddddddddddd),
        learner_id,
        session_ts,
        fingerprint
            .observation_envelopes
            .iter()
            .map(|envelope| envelope.observation_id)
            .collect(),
        "thresholds-default-pending-calibration-v1".into(),
        output_hash.clone(),
    )?;
    let delta_log = LearnerDeltaLog {
        learner_id,
        session_ts,
        computation,
        provenance,
    };
    let m_trace = update_m_trace(
        None,
        learner_id,
        trace_id,
        session_ts,
        &delta_log.computation,
        Some(true),
    )?;
    let audit_entry = LearnerAuditEntry {
        audit_id: Uuid::from_u128(0xeeeeeeee_eeee_4eee_8eee_eeeeeeeeeeee),
        learner_id,
        ts: session_ts,
        action: "record_synthetic_utl_session".into(),
        target_cf: "learner_delta_log".into(),
        result_sha256: output_hash,
        parent_audit_id: None,
    };

    Ok(SyntheticRecords {
        profile,
        fingerprint,
        delta_log,
        m_trace,
        audit_entry,
    })
}

fn source_of_truth() -> serde_json::Value {
    json!({
        "backend": "rocksdb",
        "format": "version_byte + bincode",
        "column_families": [
            "learner_profile",
            "fingerprints_learner",
            "learner_state_history",
            "learner_delta_log",
            "learner_m_per_trace",
            "learner_audit"
        ]
    })
}

fn render_profile(profile: &LearnerProfile) -> serde_json::Value {
    json!({
        "learner_id": profile.learner_id.to_string(),
        "handle": profile.handle,
        "consent_state": profile.consent_state,
        "modalities_enabled": profile.modalities_enabled.iter().map(|m| m.as_str()).collect::<Vec<_>>(),
        "calibration_session_ts": profile.calibration_session_ts,
    })
}

fn render_fingerprint(fingerprint: &LearnerFingerprint) -> serde_json::Value {
    json!({
        "learner_id": fingerprint.learner_id.to_string(),
        "session_ts": fingerprint.session_ts,
        "observation_envelopes": fingerprint.observation_envelopes.len(),
        "modality_embeddings": fingerprint.modality_embeddings.iter().map(|m| json!({
            "modality": m.modality.as_str(),
            "vector_len": m.vector.len(),
            "scalar": m.scalar,
            "source_observation_id": m.source_observation_id.to_string(),
        })).collect::<Vec<_>>(),
        "state_vector_len": fingerprint.state_vector.values.len(),
        "components": {
            "plasticity_window": fingerprint.state_vector.components.plasticity_window,
            "hrv_coherence": fingerprint.state_vector.components.hrv_coherence,
            "valence": fingerprint.state_vector.components.valence,
            "arousal": fingerprint.state_vector.components.arousal,
            "stress_floor": fingerprint.state_vector.components.stress_floor,
            "k_sleep": fingerprint.state_vector.components.k_sleep,
        }
    })
}

fn render_delta_log(log: &LearnerDeltaLog) -> serde_json::Value {
    json!({
        "learner_id": log.learner_id.to_string(),
        "session_ts": log.session_ts,
        "l": log.computation.l,
        "diagnostic_state": log.computation.diagnostic_state.as_str(),
        "delta_s": log.computation.delta_s,
        "delta_c": log.computation.delta_c,
        "delta_e": log.computation.delta_e,
        "provenance": {
            "computation_id": log.provenance.computation_id.to_string(),
            "parent_observation_ids": log.provenance.parent_observation_ids.iter().map(ToString::to_string).collect::<Vec<_>>(),
            "threshold_version": log.provenance.threshold_version,
            "output_sha256": log.provenance.output_sha256,
        }
    })
}

fn render_m_trace(trace: &context_graph_core::learner::LearnerMTrace) -> serde_json::Value {
    json!({
        "learner_id": trace.learner_id.to_string(),
        "trace_id": trace.trace_id.to_string(),
        "m_value": trace.m_value,
        "last_update_ts": trace.last_update_ts,
        "decay_rate": trace.decay_rate,
        "num_retrievals": trace.num_retrievals,
        "next_review_ts": trace.next_review_ts,
    })
}
